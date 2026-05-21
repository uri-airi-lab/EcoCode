"""Thin HTTP client for the EcoCode ingestion API. Uses only stdlib."""

from __future__ import annotations

import json
import urllib.request
from typing import Any, Optional


class EcoCodeHTTPClient:
    """Blocking HTTP client for the ingestion endpoints. Zero external deps."""

    def __init__(self, base_url: str, api_key: str) -> None:
        self._base_url = base_url.rstrip("/")
        
        if self._base_url.lower().startswith("http://"):
            import urllib.parse
            import warnings
            parsed = urllib.parse.urlparse(self._base_url)
            hostname = parsed.hostname or ""
            if hostname.lower() not in ("localhost", "127.0.0.1", "::1"):
                warnings.warn(
                    f"Insecure connection warning: The target API url '{self._base_url}' uses unencrypted HTTP. "
                    "Credentials and data will be transmitted in plaintext over the network.",
                    UserWarning,
                    stacklevel=2,
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
        with urllib.request.urlopen(req, timeout=10) as resp:
            if resp.status in (200, 201):
                raw = resp.read()
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
