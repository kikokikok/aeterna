/**
 * Create-Tenant Wizard — multi-step form that composes a `TenantManifest`
 * client-side and submits via `POST /api/v1/admin/tenants/provision`.
 *
 * §12.1, §12.7, §12.8 — harden-tenant-provisioning.
 */
import { useState } from "react"
import { useMutation, useQueryClient } from "@tanstack/react-query"
import { Loader2, X, ChevronLeft, ChevronRight } from "lucide-react"
import { cn } from "@/lib/utils"
import {
  emptyManifest,
  pruneManifest,
  manifestToYaml,
  type TenantManifest,
} from "@/api/tenant-manifest"
import { getStoredTokens } from "@/auth/token-manager"
import {
  IdentityStep,
  SecretsStep,
  HierarchyStep,
  RolesStep,
  ProvidersStep,
} from "./steps"

interface StepDef {
  id: string
  label: string
  Component: React.ComponentType<{
    manifest: TenantManifest
    onChange: (m: TenantManifest) => void
  }>
  /** Returns null when valid, error string otherwise. */
  validate: (m: TenantManifest) => string | null
}

const STEPS: StepDef[] = [
  {
    id: "identity",
    label: "Identity",
    Component: IdentityStep,
    validate: (m) => {
      if (!m.tenant.slug.trim()) return "Slug is required"
      if (!/^[a-z0-9-]+$/.test(m.tenant.slug))
        return "Slug must be lowercase letters, digits, hyphens"
      if (!m.tenant.name.trim()) return "Display name is required"
      return null
    },
  },
  {
    id: "secrets",
    label: "Secrets",
    Component: SecretsStep,
    validate: (m) => {
      const refs = m.config?.secretReferences ?? {}
      for (const [k, ref] of Object.entries(refs)) {
        switch (ref.kind) {
          case "env":
            if (!ref.var?.trim()) return `Reference "${k}" is missing env var name`
            break
          case "file":
            if (!ref.path?.trim()) return `Reference "${k}" is missing file path`
            else if (!ref.path.startsWith("/"))
              return `Reference "${k}" file path must be absolute`
            break
          case "k8s":
            if (!ref.name?.trim() || !ref.key?.trim())
              return `Reference "${k}" is missing k8s name or key`
            break
          case "postgres":
            if (!ref.secretId?.trim()) return `Reference "${k}" is missing secret id`
            break
          case "inline":
            if (!ref.inline) return `Reference "${k}" is missing inline value`
            break
        }
      }
      return null
    },
  },
  { id: "hierarchy", label: "Hierarchy", Component: HierarchyStep, validate: () => null },
  { id: "roles", label: "Roles", Component: RolesStep, validate: () => null },
  { id: "providers", label: "Providers", Component: ProvidersStep, validate: () => null },
]

interface ProvisionStepResult {
  step: string
  status: "ok" | "error" | "skipped"
  message?: string
}

interface ProvisionResponse {
  success: boolean
  status?: string
  steps?: ProvisionStepResult[]
  error?: string
  message?: string
  offendingSecrets?: string[]
}

/**
 * Submit the manifest using the apiClient pattern but with the
 * `X-Aeterna-Client-Kind: ui` header (§12.8) and capturing the typed
 * response shape.
 */
async function submitProvision(
  manifest: TenantManifest,
  opts: { allowInline: boolean },
): Promise<ProvisionResponse> {
  const tokens = getStoredTokens()
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    "X-Aeterna-Client-Kind": "ui",
  }
  if (tokens) headers["Authorization"] = `Bearer ${tokens.access_token}`
  const url = opts.allowInline
    ? "/api/v1/admin/tenants/provision?allowInline=true"
    : "/api/v1/admin/tenants/provision"
  const res = await fetch(url, {
    method: "POST",
    headers,
    body: JSON.stringify(pruneManifest(manifest)),
  })
  let body: ProvisionResponse
  try {
    body = (await res.json()) as ProvisionResponse
  } catch {
    body = { success: res.ok, error: `HTTP ${res.status}` }
  }
  if (!res.ok && body.success !== false) body.success = false
  return body
}

export function CreateTenantWizard({
  open,
  onClose,
}: {
  open: boolean
  onClose: () => void
}) {
  const queryClient = useQueryClient()
  const [manifest, setManifest] = useState<TenantManifest>(emptyManifest())
  const [stepIdx, setStepIdx] = useState(0)
  const [stepError, setStepError] = useState<string | null>(null)
  const [showPreview, setShowPreview] = useState(false)
  const [allowInline, setAllowInline] = useState(false)
  const [result, setResult] = useState<ProvisionResponse | null>(null)

  const provision = useMutation({
    mutationFn: () => submitProvision(manifest, { allowInline }),
    onSuccess: (resp) => {
      setResult(resp)
      if (resp.success) {
        queryClient.invalidateQueries({ queryKey: ["tenants"] })
      }
    },
  })

  if (!open) return null

  const currentStep = STEPS[stepIdx]
  const isLastStep = stepIdx === STEPS.length - 1
  const StepComponent = currentStep.Component

  const goNext = () => {
    const err = currentStep.validate(manifest)
    if (err) {
      setStepError(err)
      return
    }
    setStepError(null)
    if (isLastStep) {
      setShowPreview(true)
    } else {
      setStepIdx(stepIdx + 1)
    }
  }

  const goBack = () => {
    setStepError(null)
    if (showPreview) {
      setShowPreview(false)
      return
    }
    if (stepIdx > 0) setStepIdx(stepIdx - 1)
  }

  const handleClose = () => {
    setManifest(emptyManifest())
    setStepIdx(0)
    setStepError(null)
    setShowPreview(false)
    setResult(null)
    onClose()
  }

  const pruned = pruneManifest(manifest)
  const yaml = manifestToYaml(pruned)
  const hasInlineRefs = Object.values(manifest.config?.secretReferences ?? {}).some(
    (r) => r.kind === "inline" && (r.inline ?? "").length > 0,
  )

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4"
      role="dialog"
      aria-label="Create tenant wizard"
    >
      <div className="flex max-h-[90vh] w-full max-w-3xl flex-col rounded-lg bg-white shadow-xl dark:bg-gray-800">
        <div className="flex items-center justify-between border-b border-gray-200 p-4 dark:border-gray-700">
          <h2 className="text-lg font-semibold text-gray-900 dark:text-gray-100">
            Create Tenant
          </h2>
          <button onClick={handleClose} className="text-gray-400 hover:text-gray-600" aria-label="Close wizard">
            <X className="h-5 w-5" />
          </button>
        </div>

        {!result && (
          <Stepper currentIdx={stepIdx} previewActive={showPreview} />
        )}

        <div className="flex-1 overflow-y-auto p-6">
          {result ? (
            <ResultPanel result={result} onClose={handleClose} />
          ) : showPreview ? (
            <PreviewPanel
              yaml={yaml}
              hasInlineRefs={hasInlineRefs}
              allowInline={allowInline}
              setAllowInline={setAllowInline}
              isPending={provision.isPending}
              onSubmit={() => provision.mutate()}
              error={
                provision.isError
                  ? (provision.error as Error)?.message ?? "Submit failed"
                  : null
              }
            />
          ) : (
            <>
              <StepComponent manifest={manifest} onChange={setManifest} />
              {stepError && (
                <p className="mt-3 text-sm text-red-600" data-testid="wizard-step-error">
                  {stepError}
                </p>
              )}
            </>
          )}
        </div>

        {!result && (
          <div className="flex items-center justify-between border-t border-gray-200 p-4 dark:border-gray-700">
            <button
              type="button"
              onClick={goBack}
              disabled={stepIdx === 0 && !showPreview}
              className="inline-flex items-center gap-1 rounded-md border border-gray-300 px-3 py-2 text-sm hover:bg-gray-50 disabled:opacity-50 dark:border-gray-600 dark:hover:bg-gray-700"
            >
              <ChevronLeft className="h-4 w-4" /> Back
            </button>
            <span className="text-xs text-gray-500" data-testid="wizard-position">
              {showPreview
                ? `Preview`
                : `Step ${stepIdx + 1} of ${STEPS.length}: ${currentStep.label}`}
            </span>
            {!showPreview ? (
              <button
                type="button"
                onClick={goNext}
                className="inline-flex items-center gap-1 rounded-md bg-blue-600 px-3 py-2 text-sm text-white hover:bg-blue-700"
                data-testid="wizard-next"
              >
                {isLastStep ? "Preview" : "Next"} <ChevronRight className="h-4 w-4" />
              </button>
            ) : (
              <span /> /* preview submit button lives in the panel */
            )}
          </div>
        )}
      </div>
    </div>
  )
}

function Stepper({
  currentIdx,
  previewActive,
}: {
  currentIdx: number
  previewActive: boolean
}) {
  return (
    <ol className="flex items-center gap-2 border-b border-gray-200 px-4 py-3 dark:border-gray-700" aria-label="Wizard progress">
      {STEPS.map((s, i) => (
        <li key={s.id} className="flex items-center gap-2">
          <span
            className={cn(
              "flex h-6 w-6 items-center justify-center rounded-full text-xs font-medium",
              i < currentIdx || (previewActive && i <= currentIdx)
                ? "bg-blue-600 text-white"
                : i === currentIdx && !previewActive
                ? "bg-blue-100 text-blue-700 ring-2 ring-blue-600 dark:bg-blue-900 dark:text-blue-200"
                : "bg-gray-200 text-gray-500 dark:bg-gray-700",
            )}
          >
            {i + 1}
          </span>
          <span
            className={cn(
              "text-xs",
              i === currentIdx && !previewActive
                ? "font-semibold text-gray-900 dark:text-gray-100"
                : "text-gray-500",
            )}
          >
            {s.label}
          </span>
          {i < STEPS.length - 1 && <span className="text-gray-300">→</span>}
        </li>
      ))}
      <li className="ml-2 flex items-center gap-2">
        <span className="text-gray-300">→</span>
        <span
          className={cn(
            "flex h-6 w-6 items-center justify-center rounded-full text-xs font-medium",
            previewActive
              ? "bg-blue-100 text-blue-700 ring-2 ring-blue-600 dark:bg-blue-900 dark:text-blue-200"
              : "bg-gray-200 text-gray-500 dark:bg-gray-700",
          )}
        >
          ✓
        </span>
        <span className={cn("text-xs", previewActive ? "font-semibold" : "text-gray-500")}>
          Preview
        </span>
      </li>
    </ol>
  )
}

function PreviewPanel({
  yaml,
  hasInlineRefs,
  allowInline,
  setAllowInline,
  isPending,
  onSubmit,
  error,
}: {
  yaml: string
  hasInlineRefs: boolean
  allowInline: boolean
  setAllowInline: (v: boolean) => void
  isPending: boolean
  onSubmit: () => void
  error: string | null
}) {
  return (
    <div className="space-y-4" data-testid="wizard-preview">
      <p className="text-sm text-gray-600 dark:text-gray-400">
        Final manifest. Submission goes to <code className="rounded bg-gray-100 px-1 dark:bg-gray-700">POST /api/v1/admin/tenants/provision</code> with <code className="rounded bg-gray-100 px-1 dark:bg-gray-700">X-Aeterna-Client-Kind: ui</code>.
      </p>
      <pre
        className="max-h-[40vh] overflow-auto rounded-md bg-gray-900 p-4 font-mono text-xs text-gray-100"
        data-testid="wizard-yaml"
      >
        {yaml}
      </pre>
      {hasInlineRefs && (
        <label className="flex items-center gap-2 rounded-md bg-amber-50 p-3 text-sm text-amber-900 dark:bg-amber-900/20 dark:text-amber-200">
          <input
            type="checkbox"
            checked={allowInline}
            onChange={(e) => setAllowInline(e.target.checked)}
            data-testid="wizard-allow-inline"
          />
          Submit with <code className="font-mono">?allowInline=true</code> (server must also enable <code className="font-mono">provisioning.allowInlineSecret</code>)
        </label>
      )}
      {error && (
        <p className="text-sm text-red-600" data-testid="wizard-submit-error">
          {error}
        </p>
      )}
      <div className="flex justify-end">
        <button
          type="button"
          onClick={onSubmit}
          disabled={isPending}
          className="inline-flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-50"
          data-testid="wizard-submit"
        >
          {isPending && <Loader2 className="h-4 w-4 animate-spin" />}
          {isPending ? "Submitting…" : "Submit"}
        </button>
      </div>
    </div>
  )
}

function ResultPanel({
  result,
  onClose,
}: {
  result: ProvisionResponse
  onClose: () => void
}) {
  return (
    <div className="space-y-4" data-testid="wizard-result">
      <div
        className={cn(
          "rounded-md p-4 text-sm",
          result.success
            ? "bg-green-50 text-green-800 dark:bg-green-900/30 dark:text-green-300"
            : "bg-red-50 text-red-800 dark:bg-red-900/30 dark:text-red-300",
        )}
      >
        {result.success
          ? `Tenant provisioned (${result.status ?? "ok"}).`
          : `Provision failed: ${result.error ?? result.message ?? "unknown error"}`}
      </div>
      {result.offendingSecrets && result.offendingSecrets.length > 0 && (
        <div className="rounded-md bg-amber-50 p-3 text-sm text-amber-900 dark:bg-amber-900/20 dark:text-amber-200">
          Offending inline references: <code>{result.offendingSecrets.join(", ")}</code>
        </div>
      )}
      {result.steps && result.steps.length > 0 && (
        <ul className="space-y-1 text-sm" data-testid="wizard-result-steps">
          {result.steps.map((s, i) => (
            <li key={`${s.step}-${i}`} className="flex items-center gap-2">
              <span
                className={cn(
                  "inline-flex h-5 w-5 items-center justify-center rounded-full text-xs",
                  s.status === "ok"
                    ? "bg-green-200 text-green-800"
                    : s.status === "skipped"
                    ? "bg-gray-200 text-gray-700"
                    : "bg-red-200 text-red-800",
                )}
              >
                {s.status === "ok" ? "✓" : s.status === "skipped" ? "·" : "!"}
              </span>
              <span className="font-mono">{s.step}</span>
              {s.message && <span className="text-gray-500">— {s.message}</span>}
            </li>
          ))}
        </ul>
      )}
      <div className="flex justify-end">
        <button
          type="button"
          onClick={onClose}
          className="rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700"
        >
          Close
        </button>
      </div>
    </div>
  )
}
