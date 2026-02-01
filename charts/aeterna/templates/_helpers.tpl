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
