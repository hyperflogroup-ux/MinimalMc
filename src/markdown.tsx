import type { ReactNode } from 'react'
import { backend } from './backend'

// A small markdown reader for project descriptions and changelogs. It builds ordinary page elements
// (nothing is ever inserted as raw HTML), handles the parts descriptions actually use, and opens
// links in the normal browser instead of inside the launcher.

const openUrl = (url: string) => {
  if (/^https:\/\//i.test(url)) backend.openUrl(url).catch(() => {})
}

/** Turns the few kinds of HTML that descriptions contain into plain markdown, and drops the rest. */
function cleanHtml(src: string): string {
  return src
    .replace(/\r/g, '')
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<img[^>]*?src=["']([^"']+)["'][^>]*>/gi, '![]($1)')
    .replace(/<a[^>]*?href=["']([^"']+)["'][^>]*>([\s\S]*?)<\/a>/gi, '[$2]($1)')
    .replace(/<h([1-6])[^>]*>([\s\S]*?)<\/h\1>/gi, (_m, n, t) => `\n\n${'#'.repeat(Number(n))} ${t}\n\n`)
    .replace(/<li[^>]*>/gi, '\n- ')
    .replace(/<\/?(p|div|center|ul|ol|table|tr|thead|tbody|details|summary|blockquote)[^>]*>/gi, '\n\n')
    .replace(/<\/?(span|strong|em|b|i|u|s|kbd|sup|sub|td|th|font|section|article|figure|figcaption|iframe)[^>]*>/gi, ' ')
    .replace(/<[^>\n]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, '&')
}

// image | link (text may hold an image) | code | bold | bold | italic | bare address
const INLINE =
  /!\[([^\]]*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|\[((?:[^[\]]|\[[^\]]*\])*)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|`([^`]+)`|\*\*([^*]+)\*\*|__([^_]+)__|\*([^*\n]+)\*|(https?:\/\/[^\s<>)\]]+)/g

function inline(text: string, base: string): ReactNode[] {
  const out: ReactNode[] = []
  let last = 0
  let n = 0
  const re = new RegExp(INLINE.source, 'g')
  let m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) out.push(text.slice(last, m.index))
    const key = `${base}-${n++}`
    if (m[2] !== undefined) {
      out.push(/^https?:\/\//i.test(m[2])
        ? <img key={key} src={m[2]} alt={m[1]} loading="lazy" className="max-w-full rounded-lg my-1 inline-block align-middle" />
        : '')
    } else if (m[4] !== undefined) {
      const url = m[4]
      out.push(
        <a key={key} href={url} onClick={(e) => { e.preventDefault(); openUrl(url) }} className="text-act underline cursor-pointer">
          {inline(m[3], key)}
        </a>,
      )
    } else if (m[5] !== undefined) {
      out.push(<code key={key} className="px-1.5 py-0.5 rounded-md bg-s3 text-[0.78rem] font-mono">{m[5]}</code>)
    } else if (m[6] !== undefined || m[7] !== undefined) {
      out.push(<strong key={key} className="font-semibold">{inline((m[6] ?? m[7]) as string, key)}</strong>)
    } else if (m[8] !== undefined) {
      out.push(<em key={key}>{inline(m[8], key)}</em>)
    } else if (m[9] !== undefined) {
      const url = m[9]
      out.push(<a key={key} href={url} onClick={(e) => { e.preventDefault(); openUrl(url) }} className="text-act underline cursor-pointer break-all">{url}</a>)
    }
    last = m.index + m[0].length
  }
  if (last < text.length) out.push(text.slice(last))
  return out
}

function blocks(src: string): ReactNode[] {
  const lines = cleanHtml(src).split('\n')
  const out: ReactNode[] = []
  let i = 0
  let k = 0
  const key = () => `b${k++}`
  while (i < lines.length) {
    const line = lines[i]
    const t = line.trim()
    if (!t) { i++; continue }

    if (t.startsWith('```')) {
      const code: string[] = []
      i++
      while (i < lines.length && !lines[i].trim().startsWith('```')) code.push(lines[i++])
      i++
      out.push(<pre key={key()} className="rounded-xl bg-s2 p-3 text-[0.75rem] font-mono overflow-auto whitespace-pre">{code.join('\n')}</pre>)
      continue
    }
    const h = /^(#{1,6})\s+(.*)$/.exec(t)
    if (h) {
      const size = ['text-[1.25rem]', 'text-[1.125rem]', 'text-[1rem]', 'text-[0.9375rem]', 'text-[0.875rem]', 'text-[0.8438rem]'][h[1].length - 1]
      out.push(<div key={key()} className={`${size} font-semibold tracking-tight mt-2`}>{inline(h[2], `h${k}`)}</div>)
      i++
      continue
    }
    if (/^(-{3,}|\*{3,}|_{3,})$/.test(t)) { out.push(<hr key={key()} className="border-line" />); i++; continue }
    if (t.startsWith('>')) {
      const q: string[] = []
      while (i < lines.length && lines[i].trim().startsWith('>')) q.push(lines[i++].trim().replace(/^>\s?/, ''))
      out.push(<div key={key()} className="border-l-2 border-line pl-3 text-mu">{inline(q.join(' '), `q${k}`)}</div>)
      continue
    }
    if (/^([-*+]|\d+\.)\s+/.test(t)) {
      const ordered = /^\d+\./.test(t)
      const items: string[] = []
      while (i < lines.length && /^([-*+]|\d+\.)\s+/.test(lines[i].trim())) items.push(lines[i++].trim().replace(/^([-*+]|\d+\.)\s+/, ''))
      const lis = items.map((x, j) => <li key={j}>{inline(x, `l${k}-${j}`)}</li>)
      out.push(ordered
        ? <ol key={key()} className="list-decimal pl-5 space-y-1">{lis}</ol>
        : <ul key={key()} className="list-disc pl-5 space-y-1">{lis}</ul>)
      continue
    }
    if (t.startsWith('|')) {
      const rows: string[][] = []
      while (i < lines.length && lines[i].trim().startsWith('|')) {
        const cells = lines[i].trim().replace(/^\||\|$/g, '').split('|').map((c) => c.trim())
        if (!cells.every((c) => /^:?-{2,}:?$/.test(c))) rows.push(cells)
        i++
      }
      out.push(
        <div key={key()} className="overflow-auto">
          <table className="text-[0.78rem] border-collapse">
            <tbody>
              {rows.map((r, a) => (
                <tr key={a}>{r.map((c, b) => <td key={b} className={`border border-line px-2.5 py-1.5 ${a === 0 ? 'font-semibold' : ''}`}>{inline(c, `t${k}-${a}-${b}`)}</td>)}</tr>
              ))}
            </tbody>
          </table>
        </div>,
      )
      continue
    }
    // A paragraph: lines up to the next blank line or block start.
    const para: string[] = []
    while (i < lines.length) {
      const l = lines[i].trim()
      if (!l || l.startsWith('```') || /^#{1,6}\s/.test(l) || l.startsWith('>') || l.startsWith('|') || /^([-*+]|\d+\.)\s+/.test(l)) break
      para.push(l)
      i++
    }
    if (!para.length) { para.push(t); i++ }
    out.push(<p key={key()}>{inline(para.join(' '), `p${k}`)}</p>)
  }
  return out
}

export function Markdown({ text }: { text: string }) {
  return <div className="text-[0.8438rem] leading-relaxed space-y-3 break-words">{blocks(text || '')}</div>
}
