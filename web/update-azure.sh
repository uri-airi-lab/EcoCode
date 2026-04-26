#!/usr/bin/env bash
# EcoCode — redeploy changed services to existing Azure Container Apps
#
# Usage:
#   ./update-azure.sh           # rebuild & redeploy both backend and frontend
#   ./update-azure.sh backend   # backend only
#   ./update-azure.sh frontend  # frontend only

set -euo pipefail

# ── Must match deploy-azure.sh ────────────────────────────────────────────────
RESOURCE_GROUP="ecocode-rg"
ACR_NAME="ecocoderegistry"
BACKEND_APP="backend"
FRONTEND_APP="frontend"

TARGET="${1:-both}"

log() { echo; echo "▶  $*"; }

ACR_LOGIN_SERVER=$(az acr show --name "$ACR_NAME" --query loginServer -o tsv)
BACKEND_IMAGE="$ACR_LOGIN_SERVER/$BACKEND_APP:latest"
FRONTEND_IMAGE="$ACR_LOGIN_SERVER/$FRONTEND_APP:latest"

log "Logging in to ACR"
az acr login --name "$ACR_NAME"

# ── Backend ───────────────────────────────────────────────────────────────────
if [[ "$TARGET" == "backend" || "$TARGET" == "both" ]]; then
  log "Building backend image"
  docker build -t "$BACKEND_IMAGE" "$(dirname "$0")/backend"

  log "Pushing backend image"
  docker push "$BACKEND_IMAGE"

  log "Updating backend container app"
  az containerapp update \
    --name "$BACKEND_APP" \
    --resource-group "$RESOURCE_GROUP" \
    --image "$BACKEND_IMAGE" \
    --output none

  log "Backend updated"
fi

# ── Frontend ──────────────────────────────────────────────────────────────────
if [[ "$TARGET" == "frontend" || "$TARGET" == "both" ]]; then
  BACKEND_INTERNAL_URL="http://$BACKEND_APP"

  log "Building frontend image"
  docker build \
    --build-arg "NEXT_PUBLIC_API_URL=$BACKEND_INTERNAL_URL" \
    -t "$FRONTEND_IMAGE" \
    "$(dirname "$0")/frontend"

  log "Pushing frontend image"
  docker push "$FRONTEND_IMAGE"

  log "Updating frontend container app"
  az containerapp update \
    --name "$FRONTEND_APP" \
    --resource-group "$RESOURCE_GROUP" \
    --image "$FRONTEND_IMAGE" \
    --output none

  log "Frontend updated"
fi

echo
echo "════════════════════════════════════════════════════════"
echo "  Update complete"

if [[ "$TARGET" == "frontend" || "$TARGET" == "both" ]]; then
  FRONTEND_FQDN=$(az containerapp show \
    --name "$FRONTEND_APP" \
    --resource-group "$RESOURCE_GROUP" \
    --query properties.configuration.ingress.fqdn -o tsv)
  echo "  Frontend : https://$FRONTEND_FQDN"
fi

if [[ "$TARGET" == "backend" || "$TARGET" == "both" ]]; then
  BACKEND_FQDN=$(az containerapp show \
    --name "$BACKEND_APP" \
    --resource-group "$RESOURCE_GROUP" \
    --query properties.configuration.ingress.fqdn -o tsv)
  echo "  Backend  : https://$BACKEND_FQDN"
fi

echo "════════════════════════════════════════════════════════"
