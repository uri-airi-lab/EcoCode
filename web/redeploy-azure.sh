#!/usr/bin/env bash
# Quick redeploy script for Azure Container Apps
# This rebuilds and pushes only the images without recreating infrastructure

set -euo pipefail

# Configuration
RESOURCE_GROUP="ecocode-rg"
ACR_NAME="ecocoderegistry"
BACKEND_APP="backend"
FRONTEND_APP="frontend"

# Helper
log() { echo; echo "▶  $*"; }

# Get ACR login server
ACR_LOGIN_SERVER=$(az acr show --name "$ACR_NAME" --query loginServer -o tsv)
BACKEND_IMAGE="$ACR_LOGIN_SERVER/$BACKEND_APP:latest"
FRONTEND_IMAGE="$ACR_LOGIN_SERVER/$FRONTEND_APP:latest"

# Get backend URL for frontend build
BACKEND_FQDN=$(az containerapp show \
  --name "$BACKEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --query properties.configuration.ingress.fqdn -o tsv)
BACKEND_INTERNAL_URL="http://$BACKEND_APP"

log "Logging in to ACR"
az acr login --name "$ACR_NAME"

log "Building and pushing backend image"
docker build -t "$BACKEND_IMAGE" "$(dirname "$0")/backend"
docker push "$BACKEND_IMAGE"

log "Building and pushing frontend image"
docker build \
  --build-arg "NEXT_PUBLIC_API_URL=$BACKEND_INTERNAL_URL" \
  -t "$FRONTEND_IMAGE" \
  "$(dirname "$0")/frontend"
docker push "$FRONTEND_IMAGE"

log "Updating backend container app"
az containerapp update \
  --name "$BACKEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --image "$BACKEND_IMAGE" \
  --output none

log "Updating frontend container app"
az containerapp update \
  --name "$FRONTEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --image "$FRONTEND_IMAGE" \
  --output none

FRONTEND_FQDN=$(az containerapp show \
  --name "$FRONTEND_APP" \
  --resource-group "$RESOURCE_GROUP" \
  --query properties.configuration.ingress.fqdn -o tsv)

echo
echo "════════════════════════════════════════════════════════"
echo "  EcoCode redeployed successfully"
echo ""
echo "  Frontend: https://$FRONTEND_FQDN"
echo "  Backend:  https://$BACKEND_FQDN"
echo "════════════════════════════════════════════════════════"