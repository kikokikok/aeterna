{{/*
Expand the name of the chart.
*/}}
{{- define "prereqs.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "prereqs.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "prereqs.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
{{- end }}

{{/*
Return PostgreSQL host.
CloudNativePG creates a service named <cluster>-rw.
*/}}
{{- define "prereqs.postgresql.host" -}}
{{- printf "%s-cnpg-rw" (include "prereqs.fullname" .) }}
{{- end }}

{{/*
Return PostgreSQL secret name.
CloudNativePG creates a secret named <cluster>-app.
*/}}
{{- define "prereqs.postgresql.secretName" -}}
{{- printf "%s-cnpg-app" (include "prereqs.fullname" .) }}
{{- end }}

{{/*
Return Redis/cache host.
*/}}
{{- define "prereqs.redis.host" -}}
{{- if .Values.cache.dragonfly.enabled }}
{{- printf "%s-dragonfly" (include "prereqs.fullname" .) }}
{{- else if .Values.cache.valkey.enabled }}
{{- printf "%s-valkey" (include "prereqs.fullname" .) }}
{{- else }}
{{- "localhost" }}
{{- end }}
{{- end }}

{{/*
Return vector store host.
*/}}
{{- define "prereqs.vectorStore.host" -}}
{{- if .Values.vectorStore.qdrant.enabled }}
{{- printf "%s-qdrant" (include "prereqs.fullname" .) }}
{{- else }}
{{- "" }}
{{- end }}
{{- end }}

{{/*
Return vector store port.
*/}}
{{- define "prereqs.vectorStore.port" -}}
{{- if .Values.vectorStore.qdrant.enabled }}
{{- 6333 }}
{{- else }}
{{- 0 }}
{{- end }}
{{- end }}

{{/*
Return connection secret name.
*/}}
{{- define "prereqs.connectionSecretName" -}}
{{- if .Values.connectionSecret.name }}
{{- .Values.connectionSecret.name }}
{{- else }}
{{- printf "%s-connections" (include "prereqs.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Resource calculation helper (same as main chart).
*/}}
{{- define "prereqs.resources.calculate" -}}
{{- $resources := .resources | default dict -}}
{{- $limits := $resources.limits | default dict -}}
{{- $requests := $resources.requests | default dict -}}
limits:
  {{- if $limits.cpu }}
  cpu: {{ $limits.cpu }}
  {{- else if $requests.cpu }}
  cpu: {{ $requests.cpu }}
  {{- end }}
  {{- if $limits.memory }}
  memory: {{ $limits.memory }}
  {{- else if $requests.memory }}
  memory: {{ $requests.memory }}
  {{- end }}
requests:
  {{- if $requests.cpu }}
  cpu: {{ $requests.cpu }}
  {{- else if $limits.cpu }}
  cpu: {{ $limits.cpu }}
  {{- end }}
  {{- if $requests.memory }}
  memory: {{ $requests.memory }}
  {{- else if $limits.memory }}
  memory: {{ $limits.memory }}
  {{- end }}
{{- end }}

{{/*
OpenBao bootstrap Secret name (consumed by Aeterna chart).
*/}}
{{- define "prereqs.openbao.bootstrapSecretName" -}}
{{- if .Values.openbao.bootstrap.secretName }}
{{- .Values.openbao.bootstrap.secretName }}
{{- else }}
{{- printf "%s-openbao-bootstrap" (include "prereqs.fullname" .) | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}

{{/*
OpenBao bootstrap ServiceAccount name.
*/}}
{{- define "prereqs.openbao.bootstrapSA" -}}
{{- if .Values.openbao.bootstrap.serviceAccount.name }}
{{- .Values.openbao.bootstrap.serviceAccount.name }}
{{- else }}
{{- printf "%s-openbao-bootstrap" (include "prereqs.fullname" .) | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}

{{/*
In-cluster OpenBao address. Upstream chart names its service `<release>-openbao`
where `<release>` is the *subchart* release — i.e. parent .Release.Name.
*/}}
{{- define "prereqs.openbao.addr" -}}
{{- printf "http://%s-openbao.%s.svc:8200" .Release.Name .Release.Namespace }}
{{- end }}

{{/*
Guardrail: validate that openbao.mode is supported and that the upstream subchart
values are consistent with the chosen mode. Fails `helm template/install` early
with an actionable message if the contract is broken.
*/}}
{{- define "prereqs.openbao.assertSealMode" -}}
{{- $mode := .Values.openbao.mode | default "internal-dev-seal" -}}
{{- $valid := list "internal-dev-seal" "internal-shamir" "external" -}}
{{- if not (has $mode $valid) -}}
{{- fail (printf "openbao.mode=%q is invalid. Must be one of: %s" $mode (join ", " $valid)) -}}
{{- end -}}
{{- $dev := (((.Values.openbao).server).dev).enabled | default false -}}
{{- $standalone := (((.Values.openbao).server).standalone).enabled | default false -}}
{{- if eq $mode "internal-dev-seal" -}}
{{- if not $dev -}}
{{- fail "openbao.mode=internal-dev-seal requires openbao.server.dev.enabled=true. Set it explicitly or switch mode." -}}
{{- end -}}
{{- end -}}
{{- if eq $mode "internal-shamir" -}}
{{- if $dev -}}
{{- fail "openbao.mode=internal-shamir is incompatible with openbao.server.dev.enabled=true (dev mode is in-memory and auto-unsealed). Set openbao.server.dev.enabled=false and openbao.server.standalone.enabled=true." -}}
{{- end -}}
{{- if not $standalone -}}
{{- fail "openbao.mode=internal-shamir requires openbao.server.standalone.enabled=true." -}}
{{- end -}}
{{- end -}}
{{- if eq $mode "external" -}}
{{- if $dev -}}
{{- fail "openbao.mode=external is incompatible with openbao.server.dev.enabled=true. Disable dev mode and configure an auto-unseal seal stanza in openbao.server.{standalone,ha}.config." -}}
{{- end -}}
{{- end -}}
{{- end }}
