"""fakescan control-plane client (optional lab harness)."""

from .client import Activity, ActivityEvent, Client, Job, LoopbackInfo, Status
from .soap import (
    SoapFault,
    acquire_dib_platen,
    acquire_jfif_pages,
    acquire_pages,
    get_scanner_status,
)
from .wsd_docs import generate_reference_docs
from .wsd_schema import body_problems, load_schema, parse_schema

__all__ = [
    "Activity",
    "ActivityEvent",
    "Client",
    "Job",
    "LoopbackInfo",
    "SoapFault",
    "Status",
    "acquire_dib_platen",
    "acquire_jfif_pages",
    "acquire_pages",
    "get_scanner_status",
    "body_problems",
    "generate_reference_docs",
    "load_schema",
    "parse_schema",
    "__version__",
]
__version__ = "0.1.0"
