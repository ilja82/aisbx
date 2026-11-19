import type { Register } from 'claude-code'

const SCRIPT = '/usr/local/lib/aisbx/statusline/subagent-statusline.sh'

export const register: Register = (on) => {
  on('ui.render', { component: 'AgentRow' }, async ($, e, next) => {
    const p = e.props
    const input = JSON.stringify({
      columns: p.columns,
      tasks: [
        {
          id: p.id,
          label: p.label,
          status: p.taskStatus,
          tokenCount: p.tokenCount,
          contextWindowSize: p.contextWindowSize,
          startTime: p.startTime,
          description: p.description,
          tokenSamples: p.tokenSamples,
        },
      ],
    })
    let r: { exitCode: number; stdout: string }
    try {
      r = await $.process.run([SCRIPT], { stdin: input, timeoutMs: 2000 })
    } catch {
      return next(e)
    }
    if (r.exitCode !== 0) return next(e)
    for (const text of r.stdout.split('\n')) {
      let row: unknown
      try {
        row = JSON.parse(text)
      } catch {
        continue
      }
      if (
        row !== null &&
        typeof row === 'object' &&
        (row as { id?: unknown }).id === p.id &&
        typeof (row as { content?: unknown }).content === 'string'
      ) {
        return next({ ...e, props: { ...p, body: (row as { content: string }).content } })
      }
    }
    return next(e)
  })
}
