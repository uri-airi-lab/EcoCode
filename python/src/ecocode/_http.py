"""Thin HTTP client for the EcoCode ingestion API. Uses only stdlib."""

from __future__ import annotations

import json
import ssl
import urllib.request
from typing import Any, Optional

# Cap on the response body, so a misbehaving backend cannot exhaust memory.
_MAX_RESPONSE_BYTES = 1_048_576  # 1 MiB

# One strict SSL context (CERT_REQUIRED + check_hostname) shared by every request.
_SSL_CONTEXT = ssl.create_default_context()

# The SDK only calls the backend to create, describe and complete a run; records
# go through the monitor. A slow backend at those moments should cost a little
# waiting, not the whole run, so the limit is generous.
_TIMEOUT_SECS = 30


# The EcoCode backend. Fixed on purpose: it cannot be changed by arguments or
# environment variables, so run data and API keys can't be sent elsewhere by
# mistake. (Same constant as the CLI's ecocode_api::BASE_URL.)
BASE_URL = "https://ecocode.harmonyhpc.io"


class EcoCodeHTTPClient:
    """Blocking HTTP client for the ingestion endpoints. Zero external deps."""

    def __init__(self, api_key: str) -> None:
        self._base_url = BASE_URL.rstrip("/")
        self._headers = {
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        }

    def _post(self, path: str, body: dict[str, Any]) -> Any:
        url = f"{self._base_url}{path}"
        data = json.dumps(body).encode()
        req = urllib.request.Request(
            url, data=data, headers=self._headers, method="POST"
        )
        # Explicit SSL context so certificate validation is always enforced;
        # the read is capped so a bad backend cannot exhaust memory.
        with urllib.request.urlopen(req, timeout=_TIMEOUT_SECS, context=_SSL_CONTEXT) as resp:
            if resp.status in (200, 201):
                raw = resp.read(_MAX_RESPONSE_BYTES)
                if raw:
                    return json.loads(raw)
            return None

    def create_run(
        self,
        project_id: int,
        name: str,
        started_at: str,
        command: Optional[str] = None,
    ) -> int:
        resp = self._post(
            "/api/ingest/run",
            {
                "project_id": project_id,
                "name": name,
                "started_at": started_at,
                "command": command,
                "exporter_type": "sdk",
            },
        )
        return resp["id"]

    def save_system_info(self, run_id: int, info: dict) -> None:
        self._post(f"/api/ingest/run/{run_id}/system-info", info)

    def complete_run(self, run_id: int, status: str, ended_at: str) -> None:
        self._post(
            f"/api/ingest/run/{run_id}/complete",
            {"status": status, "ended_at": ended_at},
        )
