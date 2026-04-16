import { ChevronLeft, ChevronRight } from "lucide-react"
import { cn } from "@/lib/utils"

export interface PaginationBarProps {
  offset: number
  limit: number
  total: number | null
  onPageChange: (newOffset: number) => void
  className?: string
}

export function PaginationBar({ offset, limit, total, onPageChange, className }: PaginationBarProps) {
  const currentPage = Math.floor(offset / limit) + 1
  const totalPages = total != null ? Math.ceil(total / limit) : null
  const hasPrev = offset > 0
  const hasNext = total != null ? offset + limit < total : true

  return (
    <div className={cn("flex items-center justify-between border-t border-gray-200 px-4 py-3 dark:border-gray-700", className)}>
      <div className="text-sm text-gray-500 dark:text-gray-400">
        Showing {offset + 1}–{total != null ? Math.min(offset + limit, total) : offset + limit}
        {total != null && <span> of {total}</span>}
      </div>

      <div className="flex items-center gap-2">
        <button
          onClick={() => onPageChange(Math.max(0, offset - limit))}
          disabled={!hasPrev}
          className="inline-flex items-center rounded-md border border-gray-300 px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-40 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800"
        >
          <ChevronLeft className="mr-1 h-4 w-4" />
          Prev
        </button>

        <span className="text-sm text-gray-600 dark:text-gray-400">
          Page {currentPage}{totalPages != null && ` of ${totalPages}`}
        </span>

        <button
          onClick={() => onPageChange(offset + limit)}
          disabled={!hasNext}
          className="inline-flex items-center rounded-md border border-gray-300 px-3 py-1.5 text-sm font-medium text-gray-700 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-40 dark:border-gray-600 dark:text-gray-300 dark:hover:bg-gray-800"
        >
          Next
          <ChevronRight className="ml-1 h-4 w-4" />
        </button>
      </div>
    </div>
  )
}
