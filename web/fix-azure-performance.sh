#!/usr/bin/env bash
# EcoCode — Azure performance fix script
# This script addresses timeout and performance issues on Azure Container Apps

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────
RESOURCE_GROUP="ecocode-rg"
ACR_NAME="ecocoderegistry"
BACKEND_APP="backend"
FRONTEND_APP="frontend"

# ── Helper ────────────────────────────────────────────────────────────────────
log() { echo; echo "▶  $*"; }

# ── Get current URLs ──────────────────────────────────────────────────────────
BACKEND_FQDN=$(az containerapp show \
  --name "$BACKEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --query properties.configuration.ingress.fqdn -o tsv)

BACKEND_EXTERNAL_URL="https://$BACKEND_FQDN"
log "Using backend URL: $BACKEND_EXTERNAL_URL"

# ── Login to ACR ──────────────────────────────────────────────────────────────
log "Logging in to ACR"
az acr login --name "$ACR_NAME"

ACR_LOGIN_SERVER=$(az acr show --name "$ACR_NAME" --query loginServer -o tsv)
BACKEND_IMAGE="$ACR_LOGIN_SERVER/$BACKEND_APP:latest"
FRONTEND_IMAGE="$ACR_LOGIN_SERVER/$FRONTEND_APP:latest"

# ── Build & push backend with performance improvements ───────────────────────
log "Building backend image with performance optimizations"
docker build -t "$BACKEND_IMAGE" "$(dirname "$0")/backend"

log "Pushing backend image"
docker push "$BACKEND_IMAGE"

# ── Build & push frontend with timeout fixes ─────────────────────────────────
log "Building frontend image with timeout fixes"
docker build \
  --build-arg "NEXT_PUBLIC_API_URL=$BACKEND_EXTERNAL_URL" \
  -t "$FRONTEND_IMAGE" \
  "$(dirname "$0")/frontend"

log "Pushing frontend image"
docker push "$FRONTEND_IMAGE"

# ── Update backend with performance settings ──────────────────────────────────
log "Updating backend container app with performance settings"
az containerapp update \
  --name "$BACKEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --image "$BACKEND_IMAGE" \
  --cpu 1.0 \
  --memory 2.0Gi \
  --min-replicas 1 \
  --max-replicas 5 \
  --output none

# ── Update frontend with timeout settings ─────────────────────────────────────
log "Updating frontend container app with timeout settings"
az containerapp update \
  --name "$FRONTEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --image "$FRONTEND_IMAGE" \
  --cpu 0.5 \
  --memory 1.0Gi \
  --min-replicas 1 \
  --max-replicas 3 \
  --set-env-vars "NEXT_PUBLIC_API_URL=$BACKEND_EXTERNAL_URL" \
  --output none

FRONTEND_FQDN=$(az containerapp show \
  --name "$FRONTEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --query properties.configuration.ingress.fqdn -o tsv)

FRONTEND_URL="https://$FRONTEND_FQDN"

# ── Update backend CORS ───────────────────────────────────────────────────────
log "Updating backend CORS_ORIGIN → $FRONTEND_URL"
az containerapp update \
  --name "$BACKEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --set-env-vars "CORS_ORIGIN=$FRONTEND_URL" \
  --output none

# ── Done ──────────────────────────────────────────────────────────────────────
echo
echo "════════════════════════════════════════════════════════"
echo "  EcoCode performance fixes applied"
echo ""
echo "  Frontend :  $FRONTEND_URL"
echo "  Backend  :  $BACKEND_EXTERNAL_URL"
echo ""
echo "  Changes applied:"
echo "  - Increased backend CPU/memory resources"
echo "  - Added frontend timeout handling"
echo "  - Fixed SSE configuration"
echo "  - Optimized container scaling"
echo "════════════════════════════════════════════════════════"