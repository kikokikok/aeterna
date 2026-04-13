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
