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
{{- $tag := .Values.aeterna.image.tag | default (printf "v%s" .Chart.AppVersion) -}}
{{- if $registry }}
{{- printf "%s/%s:%s" $registry $repository $tag }}
{{- else }}
{{- printf "%s:%s" $repository $tag }}
{{- end }}
{{- end }}

{{/*
Return PostgreSQL host (direct connection string)
*/}}
{{- define "aeterna.postgresql.host" -}}
{{- .Values.postgresql.host }}
{{- end }}

{{/*
Return PostgreSQL port
*/}}
{{- define "aeterna.postgresql.port" -}}
{{- .Values.postgresql.port | default 5432 }}
{{- end }}

{{/*
Return PostgreSQL database
*/}}
{{- define "aeterna.postgresql.database" -}}
{{- .Values.postgresql.database | default "aeterna" }}
{{- end }}

{{/*
Return PostgreSQL secret name
*/}}
{{- define "aeterna.postgresql.secretName" -}}
{{- if .Values.postgresql.existingSecret }}
{{- .Values.postgresql.existingSecret }}
{{- else }}
{{- printf "%s-postgresql" (include "aeterna.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Return PostgreSQL password key in the secret
*/}}
{{- define "aeterna.postgresql.passwordKey" -}}
{{- .Values.postgresql.passwordKey | default "password" }}
{{- end }}

{{/*
Return Redis host (direct connection string)
*/}}
{{- define "aeterna.redis.host" -}}
{{- .Values.redis.host }}
{{- end }}

{{/*
Return Redis port
*/}}
{{- define "aeterna.redis.port" -}}
{{- .Values.redis.port | default 6379 }}
{{- end }}

{{/*
Return Redis secret name
*/}}
{{- define "aeterna.redis.secretName" -}}
{{- if .Values.redis.existingSecret }}
{{- .Values.redis.existingSecret }}
{{- else }}
{{- printf "%s-redis" (include "aeterna.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Return Redis password key in the secret
*/}}
{{- define "aeterna.redis.passwordKey" -}}
{{- .Values.redis.passwordKey | default "password" }}
{{- end }}

{{/*
Return vector store host (direct connection string)
*/}}
{{- define "aeterna.vectorStore.host" -}}
{{- .Values.vectorStore.host }}
{{- end }}

{{/*
Return vector store port
*/}}
{{- define "aeterna.vectorStore.port" -}}
{{- .Values.vectorStore.port | default 6333 }}
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
Validates required connection info and deployment mode constraints.
Call from deployment or configmap to enforce constraints at render time.
Usage: {{ include "aeterna.validateConfig" . }}
*/}}
{{- define "aeterna.validateConfig" -}}
{{- if and (eq .Values.deploymentMode "remote") .Values.aeterna.enabled -}}
  {{/* In remote mode aeterna acts as thin client — warn but allow */}}
{{- end -}}
{{- if and (eq .Values.deploymentMode "hybrid") (not .Values.central.url) -}}
  {{- fail "deploymentMode=hybrid requires central.url to be set." -}}
{{- end -}}
{{- end -}}

{{/*
Return vector store type
*/}}
{{- define "aeterna.vectorStore.type" -}}
{{- .Values.vectorStore.type | default "qdrant" }}
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

{{/*
Return Okta auth secret name.
*/}}
{{- define "aeterna.okta.secretName" -}}
{{- if .Values.okta.existingSecret }}
{{- .Values.okta.existingSecret }}
{{- else }}
{{- printf "%s-okta-auth" (include "aeterna.fullname" .) }}
{{- end }}
{{- end }}

{{- define "aeterna.tenantConfigMapName" -}}
{{- printf "aeterna-tenant-%s" . -}}
{{- end }}

{{- define "aeterna.tenantSecretName" -}}
{{- printf "aeterna-tenant-%s-secret" . -}}
{{- end }}
