#!/usr/bin/env bash
# EcoCode — Azure Container Apps redeploy script
# This script redeploys with the correct SSE configuration
#
# Prerequisites:
#   - Azure CLI installed and logged in  (az login)
#   - Docker running locally
#   - Previous deployment completed successfully

set -euo pipefail

# ── Configuration ─────────────────────────────────────────────────────────────
RESOURCE_GROUP="ecocode-rg"
ACR_NAME="ecocoderegistry"
BACKEND_APP="backend"
FRONTEND_APP="frontend"

# ── Helper ────────────────────────────────────────────────────────────────────
log() { echo; echo "▶  $*"; }

# ── Get current backend URL ───────────────────────────────────────────────────
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
FRONTEND_IMAGE="$ACR_LOGIN_SERVER/$FRONTEND_APP:latest"

# ── Build & push frontend with correct API URL ───────────────────────────────
log "Building frontend image with external backend URL for SSE"
docker build \
  --build-arg "NEXT_PUBLIC_API_URL=$BACKEND_EXTERNAL_URL" \
  -t "$FRONTEND_IMAGE" \
  "$(dirname "$0")/frontend"

log "Pushing frontend image"
docker push "$FRONTEND_IMAGE"

# ── Update frontend container app ─────────────────────────────────────────────
log "Updating frontend container app"
az containerapp update \
  --name "$FRONTEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --image "$FRONTEND_IMAGE" \
  --set-env-vars "NEXT_PUBLIC_API_URL=$BACKEND_EXTERNAL_URL" \
  --output none

FRONTEND_FQDN=$(az containerapp show \
  --name "$FRONTEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --query properties.configuration.ingress.fqdn -o tsv)

FRONTEND_URL="https://$FRONTEND_FQDN"

# ── Update backend CORS to allow frontend ─────────────────────────────────────
log "Updating backend CORS_ORIGIN → $FRONTEND_URL"
az containerapp update \
  --name "$BACKEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --set-env-vars "CORS_ORIGIN=$FRONTEND_URL" \
  --output none

# ── Done ──────────────────────────────────────────────────────────────────────
echo
echo "════════════════════════════════════════════════════════"
echo "  EcoCode redeployed with SSE fix"
echo ""
echo "  Frontend :  $FRONTEND_URL"
echo "  Backend  :  $BACKEND_EXTERNAL_URL"
echo ""
echo "  SSE should now work properly!"
echo "════════════════════════════════════════════════════════"