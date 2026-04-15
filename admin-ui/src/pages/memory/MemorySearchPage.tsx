import { useState } from "react"
import { useQuery, useMutation } from "@tanstack/react-query"
import { Brain, Search, Loader2, Filter, ThumbsUp, ThumbsDown } from "lucide-react"
import { cn } from "@/lib/utils"
import { apiClient } from "@/api/client"
import type { MemoryEntry, MemoryLayer } from "@/api/types"

const MEMORY_LAYERS: MemoryLayer[] = ["Agent", "User", "Session", "Project", "Team", "Org", "Company"]

const layerColor: Record<string, string> = {
  Agent: "bg-rose-100 text-rose-700 dark:bg-rose-900 dark:text-rose-300",
  User: "bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300",
  Session: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300",
  Project: "bg-amber-100 text-amber-700 dark:bg-amber-900 dark:text-amber-300",
  Team: "bg-emerald-100 text-emerald-700 dark:bg-emerald-900 dark:text-emerald-300",
  Org: "bg-cyan-100 text-cyan-700 dark:bg-cyan-900 dark:text-cyan-300",
  Company: "bg-indigo-100 text-indigo-700 dark:bg-indigo-900 dark:text-indigo-300",
}

interface SearchResult extends MemoryEntry {
  relevance_score?: number
}

function FeedbackButtons({ memoryId }: { memoryId: string }) {
  const [feedbackGiven, setFeedbackGiven] = useState<"up" | "down" | null>(null)

  const sendFeedback = useMutation({
    mutationFn: (feedback: "positive" | "negative") =>
      apiClient.post(`/api/v1/memory/${memoryId}/feedback`, { feedback }),
    onSuccess: (_, feedback) => {
      setFeedbackGiven(feedback === "positive" ? "up" : "down")
    },
  })

  return (
    <div className="flex items-center gap-1">
      <button
        onClick={(e) => {
          e.stopPropagation()
          sendFeedback.mutate("positive")
        }}
        disabled={feedbackGiven !== null}
        className={cn(
          "rounded p-1 hover:bg-gray-100 dark:hover:bg-gray-700",
          feedbackGiven === "up" && "bg-green-100 text-green-600 dark:bg-green-900 dark:text-green-400",
        )}
        title="Helpful"
      >
        <ThumbsUp className="h-3.5 w-3.5" />
      </button>
      <button
        onClick={(e) => {
          e.stopPropagation()
          sendFeedback.mutate("negative")
        }}
        disabled={feedbackGiven !== null}
        className={cn(
          "rounded p-1 hover:bg-gray-100 dark:hover:bg-gray-700",
          feedbackGiven === "down" && "bg-red-100 text-red-600 dark:bg-red-900 dark:text-red-400",
        )}
        title="Not helpful"
      >
        <ThumbsDown className="h-3.5 w-3.5" />
      </button>
    </div>
  )
}

export function Component() {
  return <MemorySearchPage />
}

export default function MemorySearchPage() {
  const [query, setQuery] = useState("")
  const [submittedQuery, setSubmittedQuery] = useState("")
  const [layerFilter, setLayerFilter] = useState<MemoryLayer | "">("")

  const { data, isLoading, error, refetch } = useQuery<{ items: SearchResult[] }>({
    queryKey: ["memory", "search", submittedQuery, layerFilter],
    queryFn: () =>
      apiClient.post("/api/v1/memory/search", {
        query: submittedQuery,
        ...(layerFilter && { layer: layerFilter }),
      }),
    enabled: submittedQuery.length > 0,
  })

  const results = data?.items ?? []

  const handleSearch = (e: React.FormEvent) => {
    e.preventDefault()
    setSubmittedQuery(query)
  }

  return (
    <div>
      <div className="mb-6 flex items-center gap-3">
        <Brain className="h-6 w-6 text-gray-400" />
        <h1 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">Memory</h1>
      </div>

      <form onSubmit={handleSearch} className="mb-6 space-y-3">
        <div className="flex gap-3">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-gray-400" />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Search memory entries..."
              className="w-full rounded-md border border-gray-300 py-2 pl-9 pr-3 text-sm focus:border-blue-500 focus:outline-none focus:ring-1 focus:ring-blue-500 dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
            />
          </div>
          <button
            type="submit"
            className="rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
          >
            Search
          </button>
        </div>
        <div className="flex items-center gap-3">
          <Filter className="h-4 w-4 text-gray-400" />
          <select
            value={layerFilter}
            onChange={(e) => setLayerFilter(e.target.value as MemoryLayer | "")}
            className="rounded-md border border-gray-300 px-3 py-1.5 text-sm dark:border-gray-600 dark:bg-gray-700 dark:text-gray-100"
          >
            <option value="">All layers</option>
            {MEMORY_LAYERS.map((l) => (
              <option key={l} value={l}>
                {l}
              </option>
            ))}
          </select>
        </div>
      </form>

      {!submittedQuery && (
        <div className="rounded-lg border border-dashed border-gray-300 p-12 text-center dark:border-gray-600">
          <Brain className="mx-auto h-12 w-12 text-gray-300 dark:text-gray-600" />
          <p className="mt-3 text-sm text-gray-500">Enter a query to search memory entries.</p>
        </div>
      )}

      {isLoading && (
        <div className="flex justify-center p-8">
          <Loader2 className="h-6 w-6 animate-spin text-gray-400" />
        </div>
      )}

      {error && (
        <div className="p-8 text-center text-red-600">
          Search failed.{" "}
          <button onClick={() => refetch()} className="underline">
            Retry
          </button>
        </div>
      )}

      {submittedQuery && !isLoading && !error && (
        <div className="space-y-3">
          {results.length === 0 ? (
            <p className="py-8 text-center text-sm text-gray-500">No results found.</p>
          ) : (
            results.map((entry) => (
              <div
                key={entry.id}
                className="rounded-lg border border-gray-200 bg-white p-4 dark:border-gray-700 dark:bg-gray-800"
              >
                <div className="flex items-start justify-between gap-3">
                  <p className="line-clamp-3 flex-1 text-sm text-gray-700 dark:text-gray-300">
                    {entry.content}
                  </p>
                  <FeedbackButtons memoryId={entry.id} />
                </div>
                <div className="mt-3 flex flex-wrap items-center gap-2">
                  <span
                    className={cn(
                      "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
                      layerColor[entry.layer] ?? "bg-gray-100 text-gray-700",
                    )}
                  >
                    {entry.layer}
                  </span>
                  <span className="text-xs text-gray-500 dark:text-gray-400">
                    Importance: {(entry.importanceScore ?? 0).toFixed(2)}
                  </span>
                  {entry.relevance_score !== undefined && (
                    <span className="text-xs text-gray-500 dark:text-gray-400">
                      Relevance: {(entry.relevance_score * 100).toFixed(0)}%
                    </span>
                  )}
                  <span className="text-xs text-gray-400">
                    {new Date(entry.createdAt).toLocaleDateString()}
                  </span>
                </div>
              </div>
            ))
          )}
        </div>
      )}
    </div>
  )
}
