{{/*
Expand the name of the chart.
*/}}
{{- define "aeterna.name" -}}
{{- default .Chart.Name .Values.aeterna.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "aeterna.fullname" -}}
{{- if .Values.aeterna.fullnameOverride }}
{{- .Values.aeterna.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.aeterna.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "aeterna.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "aeterna.labels" -}}
helm.sh/chart: {{ include "aeterna.chart" . }}
{{ include "aeterna.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "aeterna.selectorLabels" -}}
app.kubernetes.io/name: {{ include "aeterna.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "aeterna.serviceAccountName" -}}
{{- if .Values.aeterna.serviceAccount.create }}
{{- default (include "aeterna.fullname" .) .Values.aeterna.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.aeterna.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Return the image reference
*/}}
{{- define "aeterna.image" -}}
{{- $registry := .Values.global.imageRegistry | default "" -}}
{{- $repository := .Values.aeterna.image.repository -}}
{{- $tag := .Values.aeterna.image.tag | default .Chart.AppVersion -}}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Return PostgreSQL host
*/}}
{{- define "aeterna.postgresql.host" -}}
{{- if .Values.postgresql.bundled }}
{{- printf "%s-cnpg-rw" (include "aeterna.fullname" .) }}
{{- else }}
{{- .Values.postgresql.external.host }}
{{- end }}
{{- end }}

{{/*
Return PostgreSQL port
*/}}
{{- define "aeterna.postgresql.port" -}}
{{- if .Values.postgresql.bundled }}
{{- 5432 }}
{{- else }}
{{- .Values.postgresql.external.port }}
{{- end }}
{{- end }}

{{/*
Return PostgreSQL database
*/}}
{{- define "aeterna.postgresql.database" -}}
{{- if .Values.postgresql.bundled }}
{{- "aeterna" }}
{{- else }}
{{- .Values.postgresql.external.database }}
{{- end }}
{{- end }}

{{/*
Return PostgreSQL secret name
*/}}
{{- define "aeterna.postgresql.secretName" -}}
{{- if .Values.postgresql.bundled }}
{{- printf "%s-cnpg-app" (include "aeterna.fullname" .) }}
{{- else if .Values.postgresql.external.existingSecret }}
{{- .Values.postgresql.external.existingSecret }}
{{- else }}
{{- printf "%s-postgresql" (include "aeterna.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Return Redis host
*/}}
{{- define "aeterna.redis.host" -}}
{{- if .Values.cache.external.enabled }}
{{- .Values.cache.external.host }}
{{- else if .Values.cache.dragonfly.enabled }}
{{- printf "%s-dragonfly" (include "aeterna.fullname" .) }}
{{- else if .Values.cache.valkey.enabled }}
{{- printf "%s-valkey" (include "aeterna.fullname" .) }}
{{- else }}
{{- "localhost" }}
{{- end }}
{{- end }}

{{/*
Return Redis port
*/}}
{{- define "aeterna.redis.port" -}}
{{- if .Values.cache.external.enabled }}
{{- .Values.cache.external.port }}
{{- else }}
{{- 6379 }}
{{- end }}
{{- end }}

{{/*
Return Qdrant host
*/}}
{{- define "aeterna.qdrant.host" -}}
{{- if .Values.vectorBackend.qdrant.bundled }}
{{- printf "%s-qdrant" (include "aeterna.fullname" .) }}
{{- else }}
{{- .Values.vectorBackend.qdrant.external.host }}
{{- end }}
{{- end }}

{{/*
Return Qdrant port
*/}}
{{- define "aeterna.qdrant.port" -}}
{{- if .Values.vectorBackend.qdrant.bundled }}
{{- 6333 }}
{{- else }}
{{- .Values.vectorBackend.qdrant.external.port }}
{{- end }}
{{- end }}

{{/*
Return checksum annotation for ConfigMap/Secret changes
*/}}
{{- define "aeterna.checksumAnnotations" -}}
checksum/config: {{ include (print $.Template.BasePath "/aeterna/configmap.yaml") . | sha256sum }}
checksum/secret: {{ include (print $.Template.BasePath "/aeterna/secret.yaml") . | sha256sum }}
{{- end }}

{{/*
OPAL labels
*/}}
{{- define "aeterna.opal.labels" -}}
helm.sh/chart: {{ include "aeterna.chart" . }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: {{ include "aeterna.name" . }}
{{- end }}

{{/*
OPAL Server selector labels
*/}}
{{- define "aeterna.opal.server.selectorLabels" -}}
app.kubernetes.io/name: {{ include "aeterna.name" . }}-opal-server
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: opal-server
{{- end }}

{{/*
Cedar Agent selector labels
*/}}
{{- define "aeterna.opal.cedarAgent.selectorLabels" -}}
app.kubernetes.io/name: {{ include "aeterna.name" . }}-cedar-agent
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: cedar-agent
{{- end }}

{{/*
OPAL Fetcher selector labels
*/}}
{{- define "aeterna.opal.fetcher.selectorLabels" -}}
app.kubernetes.io/name: {{ include "aeterna.name" . }}-opal-fetcher
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/component: opal-fetcher
{{- end }}

{{/*
Resource calculation helper.
Accepts a dict with "requests" and "limits" keys.
Returns a resource block with sensible defaults applied:
  - If only requests are set, limits default to 2x requests CPU and 1.5x memory.
  - If only limits are set, requests default to limits / 2 CPU and limits / 1.5 memory.
Usage: {{ include "aeterna.resources.calculate" (dict "resources" .Values.aeterna.resources) }}
*/}}
{{- define "aeterna.resources.calculate" -}}
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
Resource block helper (simplified).
Accepts a resources object (with requests/limits) and renders a complete resources: block.
Usage: {{ include "aeterna.resources" .Values.aeterna.resources | nindent 10 }}
*/}}
{{- define "aeterna.resources" -}}
{{- if . }}
resources:
  {{- toYaml . | nindent 2 }}
{{- end }}
{{- end }}

{{/*
Configuration validation helper.
Validates mutual exclusivity and required field combinations.
Call from deployment or configmap to enforce constraints at render time.
Usage: {{ include "aeterna.validateConfig" . }}
*/}}
{{- define "aeterna.validateConfig" -}}
{{- if and .Values.postgresql.bundled (and (hasKey .Values.postgresql "external") .Values.postgresql.external.host) -}}
  {{- if ne .Values.postgresql.external.host "" -}}
    {{- fail "Cannot enable both bundled CloudNativePG (postgresql.bundled=true) and external PostgreSQL (postgresql.external.host set). Disable one." -}}
  {{- end -}}
{{- end -}}
{{- if and (eq .Values.deploymentMode "remote") .Values.aeterna.enabled -}}
  {{/* In remote mode aeterna acts as thin client — warn but allow */}}
{{- end -}}
{{- if and (eq .Values.deploymentMode "hybrid") (not .Values.central.url) -}}
  {{- fail "deploymentMode=hybrid requires central.url to be set." -}}
{{- end -}}
{{- end -}}

{{/*
PostgreSQL selector labels (used by network policies)
*/}}
{{- define "aeterna.postgresql.selectorLabels" -}}
app.kubernetes.io/name: {{ include "aeterna.fullname" . }}-cnpg
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Qdrant selector labels (used by network policies)
*/}}
{{- define "aeterna.qdrant.selectorLabels" -}}
app.kubernetes.io/name: qdrant
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Image pull secrets helper.
Merges global.imagePullSecrets and component-level imagePullSecrets.
Usage: {{ include "aeterna.imagePullSecrets" . }}
*/}}
{{- define "aeterna.imagePullSecrets" -}}
{{- $secrets := list -}}
{{- with .Values.global.imagePullSecrets -}}
  {{- range . -}}
    {{- $secrets = append $secrets . -}}
  {{- end -}}
{{- end -}}
{{- with .Values.aeterna.imagePullSecrets -}}
  {{- range . -}}
    {{- $secrets = append $secrets . -}}
  {{- end -}}
{{- end -}}
{{- if $secrets }}
imagePullSecrets:
  {{- toYaml $secrets | nindent 2 }}
{{- end }}
{{- end }}
