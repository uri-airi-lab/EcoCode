"""Thin HTTP client for the EcoCode ingestion API. Uses only stdlib."""

from __future__ import annotations

import json
import ssl
import urllib.parse
import urllib.request
from typing import Any, Optional

# CS-HTTP-002: Cap response body at 1 MiB to prevent DoS via huge payloads.
_MAX_RESPONSE_BYTES = 1_048_576  # 1 MiB

# CS-HTTP-001: Reuse one strict SSL context (CERT_REQUIRED + check_hostname)
# for every request instead of creating it per call.
_SSL_CONTEXT = ssl.create_default_context()


class EcoCodeHTTPClient:
    """Blocking HTTP client for the ingestion endpoints. Zero external deps."""

    def __init__(self, base_url: str, api_key: str) -> None:
        self._base_url = base_url.rstrip("/")

        # CS-WARN-001 / CS-HTTP-001: Reject plaintext HTTP for non-localhost targets.
        # Using warnings.warn() here is insufficient because ML training scripts
        # routinely call warnings.filterwarnings("ignore"), silently swallowing
        # the alert while still transmitting credentials in plaintext.
        # A hard ValueError is the only reliable guard.
        if self._base_url.lower().startswith("http://"):
            parsed = urllib.parse.urlparse(self._base_url)
            hostname = (parsed.hostname or "").lower()
            if hostname not in ("localhost", "127.0.0.1", "::1", "[::1]"):
                raise ValueError(
                    f"Insecure HTTP endpoint '{self._base_url}' rejected. "
                    "Transmitting your API key over plaintext HTTP exposes it to "
                    "anyone on the network path. Use 'https://' instead. "
                    "HTTP is only allowed for localhost addresses "
                    "(localhost, 127.0.0.1, ::1) during local development."
                )

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
        # CS-HTTP-001: explicit context so cert validation is always enforced.
        # CS-HTTP-002: cap the read size so a bad backend can't exhaust memory.
        with urllib.request.urlopen(req, timeout=10, context=_SSL_CONTEXT) as resp:
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
