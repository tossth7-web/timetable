import { useEffect, useRef, type ReactNode } from 'react'

/** 原生 <dialog>：零 JS 依赖的居中/底部弹层，ESC 自动关闭 */
export default function Sheet({
  open,
  title,
  onClose,
  children,
}: {
  open: boolean
  title: string
  onClose: () => void
  children: ReactNode
}) {
  const ref = useRef<HTMLDialogElement>(null)

  useEffect(() => {
    const el = ref.current
    if (!el) return
    if (open && !el.open) el.showModal()
    if (!open && el.open) el.close()
  }, [open])

  return (
    <dialog ref={ref} className="sheet" onClose={onClose} onClick={(e) => e.target === ref.current && onClose()}>
      <div className="sheet-head">
        <h2>{title}</h2>
        <button className="icon" onClick={onClose} aria-label="关闭">
          ✕
        </button>
      </div>
      <div className="sheet-body">{children}</div>
    </dialog>
  )
}
