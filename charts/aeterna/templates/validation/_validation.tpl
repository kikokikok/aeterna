{{/*
Validation helpers for Aeterna Helm chart.
These templates perform pre-flight validation checks and emit
`fail` messages for invalid configurations.

Include this template from any rendered resource (e.g., NOTES.txt or configmap)
to ensure validation runs during `helm template` and `helm install`.
*/}}

{{/*
Validate required connection strings.
PostgreSQL and Redis hosts must be provided when aeterna is enabled.
Vector store host is required unless type is pgvector (uses PostgreSQL).
*/}}
{{- define "aeterna.validate.connections" -}}
{{- if .Values.aeterna.enabled -}}
  {{- if not .Values.postgresql.host -}}
    {{- fail "postgresql.host is required. Provide a PostgreSQL connection or use the aeterna-prereqs chart." -}}
  {{- end -}}
  {{- if not .Values.redis.host -}}
    {{- fail "redis.host is required. Provide a Redis-compatible cache connection or use the aeterna-prereqs chart." -}}
  {{- end -}}
  {{- if and (not .Values.vectorStore.host) (ne .Values.vectorStore.type "pgvector") (ne .Values.vectorStore.type "pinecone") (ne .Values.vectorStore.type "vertexai") (ne .Values.vectorStore.type "databricks") -}}
    {{- fail "vectorStore.host is required when vectorStore.type is not pgvector, pinecone, vertexai, or databricks." -}}
  {{- end -}}
{{- end -}}
{{- end -}}

{{/*
Validate deployment mode requires central URL for hybrid/remote.
*/}}
{{- define "aeterna.validate.deploymentMode" -}}
{{- if and (ne .Values.deploymentMode "local") (not .Values.central.url) -}}
{{- fail (printf "Invalid configuration: deploymentMode=%s requires central.url to be set." .Values.deploymentMode) -}}
{{- end -}}
{{- end -}}

{{/*
Validate vector store type configuration.
*/}}
{{- define "aeterna.validate.vectorStore" -}}
{{- $type := .Values.vectorStore.type -}}
{{- if and (eq $type "pinecone") (not .Values.vectorStore.existingSecret) -}}
{{- fail "Invalid configuration: vectorStore.type=pinecone requires vectorStore.existingSecret for API key." -}}
{{- end -}}
{{- end -}}

{{/*
Validate LLM provider configuration.
*/}}
{{- define "aeterna.validate.llm" -}}
{{- if and (eq .Values.llm.provider "google") (not .Values.llm.google.projectId) -}}
{{- fail "Invalid configuration: llm.provider=google requires llm.google.projectId to be set." -}}
{{- end -}}
{{- if and (eq .Values.llm.provider "google") (not .Values.llm.google.location) -}}
{{- fail "Invalid configuration: llm.provider=google requires llm.google.location to be set." -}}
{{- end -}}
{{- if and (eq .Values.llm.provider "google") (not .Values.llm.google.model) -}}
{{- fail "Invalid configuration: llm.provider=google requires llm.google.model to be set." -}}
{{- end -}}
{{- if and (eq .Values.llm.provider "google") (not .Values.llm.google.embeddingModel) -}}
{{- fail "Invalid configuration: llm.provider=google requires llm.google.embeddingModel to be set." -}}
{{- end -}}
{{- if and (eq .Values.llm.provider "bedrock") (not .Values.llm.bedrock.region) -}}
{{- fail "Invalid configuration: llm.provider=bedrock requires llm.bedrock.region to be set." -}}
{{- end -}}
{{- if and (eq .Values.llm.provider "bedrock") (not .Values.llm.bedrock.model) -}}
{{- fail "Invalid configuration: llm.provider=bedrock requires llm.bedrock.model to be set." -}}
{{- end -}}
{{- if and (eq .Values.llm.provider "bedrock") (not .Values.llm.bedrock.embeddingModel) -}}
{{- fail "Invalid configuration: llm.provider=bedrock requires llm.bedrock.embeddingModel to be set." -}}
{{- end -}}
{{- end -}}

{{/*
Validate Code Search image support.
The default repository is not built by this repo's image workflow, so operators
must supply an explicit published mirror/repository when enabling the sidecar.
*/}}
{{- define "aeterna.validate.codesearch" -}}
{{- if and .Values.codesearch.enabled (eq (.Values.codesearch.image.repository | default "") "ghcr.io/kikokikok/codesearch") -}}
{{- fail "Invalid configuration: codesearch.enabled requires an explicit published codesearch.image.repository override because the default repository is not built by this chart workflow." -}}
{{- end -}}
{{- end -}}

{{/*
Validate secrets provider configuration.
*/}}
{{- define "aeterna.validate.secrets" -}}
{{- if hasKey .Values "secrets" -}}
  {{- if and (eq ((.Values.secrets).provider | default "helm") "external-secrets") -}}
    {{- if not ((.Values.secrets).externalSecrets).enabled -}}
{{- fail "Invalid configuration: secrets.provider=external-secrets requires secrets.externalSecrets.enabled=true." -}}
    {{- end -}}
  {{- end -}}
{{- end -}}
{{- end -}}

{{/*
Validate Okta-backed auth boundary configuration.
*/}}
{{- define "aeterna.validate.okta" -}}
{{- if .Values.okta.enabled -}}
  {{- if not .Values.aeterna.ingress.enabled -}}
{{- fail "Invalid configuration: okta.enabled requires aeterna.ingress.enabled=true." -}}
  {{- end -}}
  {{- if not .Values.opal.enabled -}}
{{- fail "Invalid configuration: okta.enabled requires opal.enabled=true for the supported production authorization path." -}}
  {{- end -}}
  {{- if not .Values.okta.issuerUrl -}}
{{- fail "Invalid configuration: okta.enabled requires okta.issuerUrl to be set." -}}
  {{- end -}}
  {{- if not .Values.okta.clientId -}}
{{- fail "Invalid configuration: okta.enabled requires okta.clientId to be set." -}}
  {{- end -}}
  {{- if not .Values.okta.redirectUrl -}}
{{- fail "Invalid configuration: okta.enabled requires okta.redirectUrl to be set." -}}
  {{- end -}}
  {{- if and (not .Values.okta.existingSecret) (not .Values.okta.clientSecret) -}}
{{- fail "Invalid configuration: okta.enabled requires okta.clientSecret or okta.existingSecret." -}}
  {{- end -}}
{{- end -}}
{{- end -}}

{{/*
Run all validations. Include this once in a rendered resource.
*/}}
{{- define "aeterna.validate.all" -}}
{{- include "aeterna.validate.connections" . -}}
{{- include "aeterna.validate.deploymentMode" . -}}
{{- include "aeterna.validate.vectorStore" . -}}
{{- include "aeterna.validate.llm" . -}}
{{- include "aeterna.validate.codesearch" . -}}
{{- include "aeterna.validate.secrets" . -}}
{{- include "aeterna.validate.okta" . -}}
{{- end -}}
