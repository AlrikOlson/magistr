import { decode } from './wire.js'

const DIRECT = new Set([
  'context',
  'search',
  'read',
  'edit',
  'write',
  'arrange_steps',
  'start_step',
  'record_progress',
  'check_step',
  'finish_step',
])

// One bounded operation per request. The native catalog and owner authorize it.
export function request(operation, args = {}) {
  return DIRECT.has(operation)
    ? { tool: operation, args }
    : {
        tool: 'execute',
        args: {
          source:
            'return magistr.call(' +
            JSON.stringify(operation) +
            ', ' +
            JSON.stringify(args) +
            ');',
          maxOperations: 1,
        },
      }
}

export function response(operation, raw) {
  if (DIRECT.has(operation)) return raw
  const value = decode(raw)
  // A composition answers with one receipt per operation it ran.
  const receipt = value.receipts?.[0]
  if (
    !value.ok ||
    value.receipts?.length !== 1 ||
    receipt?.operation !== operation ||
    receipt.failed
  )
    throw new Error(
      'The request did not return its completed result. Refresh and inspect the record.',
    )
  return receipt.result
}
