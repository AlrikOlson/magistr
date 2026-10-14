import test from 'node:test'
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, readFile, readdir, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { createInterface } from 'node:readline'
import { decode, validate, phase } from '../hooks/wire.js'

async function verifyWorkstreams(call, lane) {
  const read = (args = {}) =>
    call('roadmap_focus_get', { lane, ...args }).then((reply) =>
      validate('focus', reply),
    )
  assert.equal((await read()).focus_state.state, 'unset')
  for (const [id, group, status] of [
    ['current', 'Rust desktop', 'pending'],
    ['earlier', 'Rust desktop', 'done'],
    ['retired', 'Rust', 'obsoleted'],
  ]) {
    const added = await call('roadmap_add_chunk', {
      id,
      title: id,
      group,
      status,
    })
    assert.notEqual(added.isError, true)
  }
  await call('roadmap_focus_set', { lane, group: 'Rust desktop', mode: 'build' })
  const current = await read()
  assert.deepEqual(Object.keys(current.workstreams.current), ['Rust desktop'])
  assert.equal(current.frontier.frontier.activity.next_ready, 'current')
  assert.equal(JSON.stringify(current).includes('chunk_ids'), false)

  const history = await read({ group: 'Rust desktop', view: 'history' })
  assert.equal(history.frontier.state, 'history')
  assert.deepEqual(history.frontier.history.chunk_ids, ['earlier'])
  assert.deepEqual(history.focus, current.focus)
  const closed = await read({ group: 'Rust', view: 'history' })
  assert.deepEqual(closed.frontier.history.chunk_ids, ['retired'])
  assert.deepEqual(closed.focus, current.focus)
  const records = decode(await call('roadmap_get', { ids: ['earlier', 'retired'] }))
  assert.deepEqual(records.records.map((record) => record.id), ['earlier', 'retired'])

  await call('roadmap_obsolete_chunk', {
    id: 'current',
    reason: 'Exercise closure of the last unfinished item',
  })
  const ended = await read()
  assert.deepEqual(ended.focus_state, { state: 'closed', group: 'Rust desktop' })
  assert.deepEqual(ended.workstreams.current, {})
  assert.equal(JSON.stringify(ended).includes('chunk_ids'), false)
  assert.deepEqual((await read({ view: 'history' })).frontier.history.chunk_ids, [
    'current',
    'earlier',
  ])
  assert.equal((await read({ lane: lane + '/another' })).focus_state.state, 'unset')
}

// Exercise actual persisted bytes while the same native owner remains connected.
async function verifyReadRecovery(call, file, readers) {
  const saved = await readFile(file)
  const before = await Promise.all(
    readers.map(([name, args]) => call(name, args).then(decode)),
  )
  try {
    await writeFile(file, '{unreadable')
    for (let attempt = 0; attempt < 2; attempt++) {
      for (const [name, args] of readers) {
        let failure
        try {
          const reply = await call(name, args)
          assert.equal(
            reply.isError,
            true,
            name + ' must refuse unreadable records',
          )
          failure = JSON.stringify(reply)
        } catch (error) {
          if (error.code === 'ERR_ASSERTION') throw error
          failure = error.message
        }
        assert.match(failure, /saved|read|invalid/i)
      }
    }
  } finally {
    await writeFile(file, saved)
  }
  for (const [index, [name, args]] of readers.entries()) {
    assert.deepEqual(decode(await call(name, args)), before[index])
  }
}

// Genuine Magistr MCP over stdio in a new fixture project, never this checkout's owner.
test(
  'native service preserves work identity and refuses unreadable persisted records',
  { timeout: 60000 },
  async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'magistr-mod-mcp-'))
    let child, lines, closed, primaryError
    try {
      const root = path.join(directory, 'project'),
        data = path.join(directory, 'data')
      await mkdir(path.join(root, '.magistr'), { recursive: true })
      await mkdir(path.join(root, '.think-and-ship'), { recursive: true })
      const projectId =
        'fixture-' + path.basename(directory).slice(-6).toLowerCase()
      await writeFile(
        path.join(root, '.think-and-ship/project.json'),
        JSON.stringify({ id: projectId }),
      )
      await writeFile(
        path.join(root, '.magistr/project.json'),
        JSON.stringify({
          project: { name: 'fixture', type: 'cli' },
          stack: {
            language: 'rust',
            framework: 'none',
            package_manager: 'cargo',
            test_framework: 'cargo-test',
            linter: 'clippy',
            formatter: 'rustfmt',
          },
          quality_gate: { check: 'echo checked', test: '', lint: '' },
          format: { command: 'echo formatted', extensions: ['rs'] },
          testing: {
            companion_pattern: '{name}.tests.rs',
            test_dir: 'alongside',
          },
          reference_repos: [],
          state_prefix: 'fixture',
          roadmap_exports: false,
        }),
      )
      const binary =
        process.env.MAGISTR_MOD_BINARY ||
        path.resolve(
          'target/debug/magistr' + (process.platform === 'win32' ? '.exe' : ''),
        )
      child = spawn(binary, ['mcp-launch', 'magistr', root], {
        cwd: root,
        env: {
          ...process.env,
          THINK_AND_SHIP_DATA_DIR: data,
          THINK_AND_SHIP_PROJECT_NAME: projectId,
          THINK_AND_SHIP_PERSIST: 'true',
          THINK_AND_SHIP_BROADCAST_PATH: '',
          MAGISTR_EXECUTION_ID: 'fixture',
        },
        stdio: ['pipe', 'pipe', 'pipe'],
        windowsHide: true,
      })
      closed = new Promise((resolve) => child.once('close', resolve))
      let stderr = ''
      child.stderr.on('data', (bytes) => {
        stderr = (stderr + bytes).slice(-3000)
      })
      const pending = new Map()
      let sequence = 0
      lines = createInterface({ input: child.stdout })
      lines.on('line', (text) => {
        const reply = JSON.parse(text)
        const waiting = pending.get(reply.id)
        if (!waiting) return
        pending.delete(reply.id)
        clearTimeout(waiting.timer)
        if (reply.error) waiting.reject(Error(reply.error.message))
        else waiting.resolve(reply.result)
      })
      const stopped = (error) => {
        for (const waiting of pending.values()) {
          clearTimeout(waiting.timer)
          waiting.reject(error)
        }
        pending.clear()
      }
      child.on('error', stopped)
      child.on('exit', (code) =>
        stopped(Error('Native owner ended (' + code + '): ' + stderr)),
      )
      function request(method, params) {
        return new Promise((resolve, reject) => {
          const id = ++sequence
          const timer = setTimeout(() => {
            pending.delete(id)
            reject(Error('MCP request timed out: ' + method + '\n' + stderr))
          }, 15000)
          pending.set(id, { resolve, reject, timer })
          child.stdin.write(
            JSON.stringify({ jsonrpc: '2.0', id, method, params }) + '\n',
          )
        })
      }
      const info = await request('initialize', {
        protocolVersion: '2025-06-18',
        capabilities: {},
        clientInfo: { name: 'magistr-mod-boundary-test', version: '1' },
      })
      assert.equal(info.serverInfo.name, 'magistr')
      child.stdin.write(
        JSON.stringify({
          jsonrpc: '2.0',
          method: 'notifications/initialized',
        }) + '\n',
      )
      const tools = await request('tools/list', {})
      assert.ok(tools.tools.some((t) => t.name === 'magistr_context'))
      const call = (name, args = {}) =>
        request('tools/call', { name, arguments: args })
      const ship = validate('ship', await call('ship_status'))
      assert.equal(ship.project_id, projectId)
      const first = validate('context', await call('magistr_context'))
      assert.equal(first.phase, null)
      validate('roadmap', await call('roadmap_status'))
      validate('findings', await call('think_engine_status', { verbose: true }))
      assert.equal(phase(first, ship), 'Phase not reported')
      await verifyWorkstreams(call, root)
      await verifyReadRecovery(
        call,
        path.join(data, 'roadmap', 'sessions', projectId + '.json'),
        [
          ['roadmap_status', {}],
          ['roadmap_focus_get', { lane: root }],
        ],
      )
      await call('ship_set_objective', {
        description: 'Inspect the connection',
        verification: 'focused',
      })
      await call('ship_plan', {
        action: 'add',
        task_id: 'verify',
        title: 'Verify the connection',
        task_type: 'test',
      })
      await call('ship_start', { task_id: 'verify' })
      await call('magistr_set_phase', { phase: 'verify' })
      const current = validate('ship', await call('ship_status'))
      const context = validate(
        'context',
        await call('magistr_context', { task_id: 'verify' }),
      )
      assert.equal(phase(context, current), 'Checking')
      assert.equal(context.task.record.title, 'Verify the connection')
      await assert.rejects(() =>
        call('magistr_context', { task_id: 'missing' }),
      )
      const gate = await call('magistr_gate', {
        gate: 'check',
        timeout_secs: 10,
      })
      const result = decode(gate)
      assert.equal(result.passed, true)
      assert.equal(result.recorded, true)
      const receipt = validate(
        'context',
        await call('magistr_context', { receipt_id: result.receipt }),
      )
      assert.equal(receipt.receipt.command, 'echo checked')
      assert.equal(receipt.receipt.settlement.settlement, 'applied')
      const executions = await readdir(path.join(data, 'ship', 'executions'))
      assert.equal(executions.length, 1)
      await verifyReadRecovery(
        call,
        path.join(
          data,
          'ship',
          'executions',
          executions[0],
          'sessions',
          projectId + '.json',
        ),
        [
          ['ship_status', {}],
          ['magistr_context', { task_id: 'verify' }],
        ],
      )
      child.stdin.end()
      await closed
    } catch (error) {
      primaryError = error
      throw error
    } finally {
      lines?.close()
      if (child && child.exitCode == null) {
        child.stdin.end()
        const settled = await Promise.race([
          closed.then(() => true),
          new Promise((resolve) => setTimeout(() => resolve(false), 2000)),
        ])
        if (!settled) {
          child.kill()
          await closed
        }
      }
      // Only this freshly created, resolved temporary fixture is removed.
      try {
        await rm(directory, {
          recursive: true,
          force: true,
          maxRetries: 10,
          retryDelay: 100,
        })
      } catch (error) {
        if (!primaryError) throw error
      }
    }
  },
)
