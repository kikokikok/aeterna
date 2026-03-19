{{/*
Validation helpers for Aeterna Helm chart.
These templates perform pre-flight validation checks and emit
`fail` messages for invalid configurations.

Include this template from any rendered resource (e.g., NOTES.txt or configmap)
to ensure validation runs during `helm template` and `helm install`.
*/}}

{{/*
Validate cache mutual exclusivity.
Only one of dragonfly and valkey may be enabled at the same time.
*/}}
{{- define "aeterna.validate.cache" -}}
{{- if and .Values.cache.dragonfly.enabled .Values.cache.valkey.enabled -}}
{{- fail "Invalid configuration: cache.dragonfly.enabled and cache.valkey.enabled cannot both be true. Choose one cache backend." -}}
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
Validate vector backend type matches an enabled backend.
*/}}
{{- define "aeterna.validate.vectorBackend" -}}
{{- $type := .Values.vectorBackend.type -}}
{{- if and (eq $type "qdrant") (not .Values.vectorBackend.qdrant.bundled) (not .Values.vectorBackend.qdrant.external.host) -}}
{{- fail "Invalid configuration: vectorBackend.type=qdrant with bundled=false requires vectorBackend.qdrant.external.host to be set." -}}
{{- end -}}
{{- if and (eq $type "pinecone") (not .Values.vectorBackend.pinecone.existingSecret) (not .Values.vectorBackend.pinecone.apiKey) -}}
{{- fail "Invalid configuration: vectorBackend.type=pinecone requires pinecone.apiKey or pinecone.existingSecret." -}}
{{- end -}}
{{- end -}}

{{/*
Validate PostgreSQL configuration.
*/}}
{{- define "aeterna.validate.postgresql" -}}
{{- if and (not .Values.postgresql.bundled) (ne .Values.deploymentMode "hybrid") (not .Values.postgresql.external.host) -}}
{{- fail "Invalid configuration: postgresql.bundled=false requires postgresql.external.host to be set." -}}
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
{{- include "aeterna.validate.cache" . -}}
{{- include "aeterna.validate.deploymentMode" . -}}
{{- include "aeterna.validate.vectorBackend" . -}}
{{- include "aeterna.validate.postgresql" . -}}
{{- include "aeterna.validate.codesearch" . -}}
{{- include "aeterna.validate.secrets" . -}}
{{- include "aeterna.validate.okta" . -}}
{{- end -}}
