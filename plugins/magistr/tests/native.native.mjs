import test from 'node:test'
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { createInterface } from 'node:readline'
import { validate, phase } from '../hooks/wire.js'

// Genuine Magistr MCP over stdio in a new fixture project, never this checkout's owner.
test(
  'shipped decoder reads the native service and its explicitly announced phase',
  { timeout: 60000 },
  async () => {
    const directory = await mkdtemp(path.join(tmpdir(), 'magistr-mod-mcp-'))
    let child, lines, closed, primaryError
    try {
      const root = path.join(directory, 'project'),
        data = path.join(directory, 'data')
      await mkdir(path.join(root, '.magistr'), { recursive: true })
      await mkdir(path.join(root, '.think-and-ship'), { recursive: true })
      await writeFile(
        path.join(root, '.think-and-ship/project.json'),
        JSON.stringify({ id: 'claude-mod-' + path.basename(directory) }),
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
        env: { ...process.env, THINK_AND_SHIP_DATA_DIR: data },
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
      const first = validate('context', await call('magistr_context'))
      assert.equal(first.phase, null)
      validate('roadmap', await call('roadmap_status'))
      validate('findings', await call('think_engine_status', { verbose: true }))
      assert.equal(phase(first, ship), 'Phase not reported')
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
      const { decode } = await import('../hooks/wire.js')
      const result = decode(gate)
      assert.equal(result.passed, true)
      assert.equal(result.recorded, true)
      const receipt = validate(
        'context',
        await call('magistr_context', { receipt_id: result.receipt }),
      )
      assert.equal(receipt.receipt.command, 'echo checked')
      assert.equal(receipt.receipt.settlement.settlement, 'applied')
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
