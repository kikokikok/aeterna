import { useParams, useNavigate } from "react-router-dom"
import { useQuery } from "@tanstack/react-query"
import { BookOpen, ArrowLeft, Loader2, GitCommit, User, Tag, Layers } from "lucide-react"
import { cn } from "@/lib/utils"
import { apiClient } from "@/api/client"
import type { KnowledgeEntry } from "@/api/types"

const typeBadgeColor: Record<string, string> = {
  Adr: "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-300",
  Policy: "bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-300",
  Pattern: "bg-teal-100 text-teal-700 dark:bg-teal-900 dark:text-teal-300",
  Spec: "bg-orange-100 text-orange-700 dark:bg-orange-900 dark:text-orange-300",
  Hindsight: "bg-pink-100 text-pink-700 dark:bg-pink-900 dark:text-pink-300",
}

export function Component() {
  return <KnowledgeDetailPage />
}

export default function KnowledgeDetailPage() {
  const { id } = useParams()
  const navigate = useNavigate()

  const { data: entry, isLoading, error, refetch } = useQuery<KnowledgeEntry>({
    queryKey: ["knowledge", id],
    queryFn: () => apiClient.get(`/api/v1/knowledge/${id}`),
    enabled: !!id,
  })

  return (
    <div>
      <div className="mb-6 flex items-center gap-3">
        <button onClick={() => navigate("/admin/knowledge")} className="text-gray-400 hover:text-gray-600">
          <ArrowLeft className="h-5 w-5" />
        </button>
        <BookOpen className="h-6 w-6 text-gray-400" />
        <h1 className="text-2xl font-semibold text-gray-900 dark:text-gray-100">Knowledge Entry</h1>
      </div>

      {isLoading && (
        <div className="flex justify-center p-8">
          <Loader2 className="h-6 w-6 animate-spin text-gray-400" />
        </div>
      )}

      {error && (
        <div className="p-8 text-center text-red-600">
          Failed to load entry.{" "}
          <button onClick={() => refetch()} className="underline">
            Retry
          </button>
        </div>
      )}

      {entry && (
        <div className="grid grid-cols-1 gap-6 md:grid-cols-3">
          {/* Content */}
          <div className="md:col-span-2">
            <div className="rounded-lg border border-gray-200 bg-white p-6 dark:border-gray-700 dark:bg-gray-800">
              <h2 className="mb-4 text-lg font-semibold text-gray-900 dark:text-gray-100">
                {entry.path || entry.id}
              </h2>
              <div className="prose prose-sm max-w-none dark:prose-invert">
                <pre className="whitespace-pre-wrap rounded-md bg-gray-50 p-4 text-sm text-gray-800 dark:bg-gray-900 dark:text-gray-200">
                  {entry.content}
                </pre>
              </div>
            </div>
          </div>

          {/* Metadata Sidebar */}
          <div className="space-y-4">
            <div className="rounded-lg border border-gray-200 bg-white p-5 dark:border-gray-700 dark:bg-gray-800">
              <h3 className="mb-3 text-sm font-medium text-gray-500 dark:text-gray-400">Metadata</h3>
              <dl className="space-y-3">
                <div>
                  <dt className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
                    <Tag className="h-3 w-3" /> Type
                  </dt>
                  <dd className="mt-1">
                    <span
                      className={cn(
                        "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
                        typeBadgeColor[entry.kind] ?? "bg-gray-100 text-gray-700",
                      )}
                    >
                      {entry.kind}
                    </span>
                  </dd>
                </div>

                <div>
                  <dt className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
                    <Layers className="h-3 w-3" /> Layer
                  </dt>
                  <dd className="mt-1 text-sm text-gray-900 dark:text-gray-100">{entry.layer}</dd>
                </div>

                <div>
                  <dt className="text-xs text-gray-500 dark:text-gray-400">Status</dt>
                  <dd className="mt-1">
                    <span
                      className={cn(
                        "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
                        entry.status === "Accepted"
                          ? "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-300"
                          : entry.status === "Draft"
                            ? "bg-yellow-100 text-yellow-700 dark:bg-yellow-900 dark:text-yellow-300"
                            : "bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300",
                      )}
                    >
                      {entry.status}
                    </span>
                  </dd>
                </div>

                {entry.author && (
                  <div>
                    <dt className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
                      <User className="h-3 w-3" /> Author
                    </dt>
                    <dd className="mt-1 text-sm text-gray-900 dark:text-gray-100">{entry.author}</dd>
                  </div>
                )}

                {entry.commit_hash && (
                  <div>
                    <dt className="flex items-center gap-1 text-xs text-gray-500 dark:text-gray-400">
                      <GitCommit className="h-3 w-3" /> Commit
                    </dt>
                    <dd className="mt-1 font-mono text-sm text-gray-900 dark:text-gray-100">
                      {entry.commit_hash.slice(0, 8)}
                    </dd>
                  </div>
                )}

                {Array.isArray(entry.metadata?.tags) && (entry.metadata.tags as string[]).length > 0 && (
                  <div>
                    <dt className="text-xs text-gray-500 dark:text-gray-400">Tags</dt>
                    <dd className="mt-1 flex flex-wrap gap-1">
                      {(entry.metadata.tags as string[]).map((tag) => {
                        const label = String(tag)
                        return (
                          <span
                            key={label}
                            className="inline-flex rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600 dark:bg-gray-700 dark:text-gray-400"
                          >
                            {label}
                          </span>
                        )
                      })}
                    </dd>
                  </div>
                )}

                <div>
                  <dt className="text-xs text-gray-500 dark:text-gray-400">Updated</dt>
                  <dd className="mt-1 text-sm text-gray-900 dark:text-gray-100">
                    {new Date(entry.updated_at).toLocaleString()}
                  </dd>
                </div>
              </dl>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
