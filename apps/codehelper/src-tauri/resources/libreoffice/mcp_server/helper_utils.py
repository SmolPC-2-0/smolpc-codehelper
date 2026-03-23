#!/usr/bin/env python
import os
import time
import traceback
import logging
import sys
from contextlib import contextmanager
from pathlib import Path


# Set up logging immediately when this module is imported
def _resolve_log_path(filename):
    """Resolve a trusted log file location."""
    configured_dir = os.getenv("SMOLPC_MCP_LOG_DIR")
    try:
        if configured_dir:
            log_dir = Path(configured_dir).expanduser()
            if not log_dir.is_absolute():
                raise ValueError("SMOLPC_MCP_LOG_DIR must be an absolute path")
            log_dir.mkdir(parents=True, exist_ok=True)
            return str(log_dir.resolve(strict=True) / filename)
    except (OSError, RuntimeError, ValueError):
        pass

    module_dir = Path(__file__).resolve().parent
    module_dir.mkdir(parents=True, exist_ok=True)
    return str(module_dir / filename)


def _setup_module_logging():
    """Internal function to set up logging for this module."""
    try:
        log_path = _resolve_log_path("helper.log")

        # Clear existing handlers
        for handler in logging.root.handlers[:]:
            logging.root.removeHandler(handler)

        # Configure logging
        logging.basicConfig(
            filename=log_path,
            level=logging.INFO,
            format="%(asctime)s %(levelname)s %(message)s",
            force=True,
            filemode="a",
        )

        # Test logging
        logging.info("=" * 50)
        logging.info(f"helper_utils module loaded - logging to: {log_path}")

    except Exception as e:
        sys.stderr.write(f"Failed to set up logging in helper_utils: {e}\n")
        logging.basicConfig(
            level=logging.INFO,
            format="%(asctime)s %(levelname)s %(message)s",
            stream=sys.stderr,
        )


# Call the setup function
_setup_module_logging()

try:
    logging.info("Importing UNO...")
    import uno
    from com.sun.star.beans import PropertyValue
    from com.sun.star.connection import NoConnectException

    logging.info("UNO imported successfully!")
except ImportError as e:
    logging.error(f"UNO Import Error: {e}")
    logging.error("This script must be run with LibreOffice's Python.")
    sys.exit(1)


class HelperError(Exception):
    pass


UNO_CONNECT_RETRIES = 2
UNO_CONNECT_RETRY_DELAY_SECONDS = 0.25
RETRYABLE_UNO_ERROR_MARKERS = (
    "binary urp bridge disposed",
    "disposedexception",
    "noconnectexception",
    "failed to connect to libreoffice desktop",
    "connection refused",
    "connector",
    "socket",
)


@contextmanager
def managed_document(file_path, read_only=False):
    doc, message = open_document(file_path, read_only)
    if not doc:
        raise HelperError(message)
    try:
        yield doc
    finally:
        try:
            doc.close(True)
        except Exception:
            pass


# Helper functions


def ensure_directory_exists(file_path):
    """Ensure the directory for a file exists, creating it if necessary."""
    directory = os.path.dirname(file_path)
    if directory and not os.path.exists(directory):
        try:
            os.makedirs(directory, exist_ok=True)
            print(f"Created directory: {directory}")
        except Exception as e:
            print(f"Failed to create directory {directory}: {str(e)}")
            return False
    return True


def normalize_path(file_path):
    """Convert a relative path to an absolute path."""
    if not file_path:
        raise HelperError("File path cannot be empty or None")

    if not isinstance(file_path, str):
        raise HelperError(f"File path must be a string, got {type(file_path).__name__}")

    # If file path is already complete, return it
    if file_path.startswith(("file://", "http://", "https://", "ftp://")):
        return file_path

    # Expand user directory if path starts with ~
    if file_path.startswith("~"):
        file_path = os.path.expanduser(file_path)

    # Make absolute if relative
    if not os.path.isabs(file_path):
        file_path = os.path.abspath(file_path)

    print(f"Normalized path: {file_path}")
    return file_path


def is_retryable_uno_error(error):
    text = f"{type(error).__name__}: {error}".lower()
    return any(marker in text for marker in RETRYABLE_UNO_ERROR_MARKERS)


def get_uno_desktop(
    retries=UNO_CONNECT_RETRIES, delay=UNO_CONNECT_RETRY_DELAY_SECONDS
):
    """Get LibreOffice desktop object."""
    last_exception = None
    for attempt in range(1, retries + 1):
        try:
            local_context = uno.getComponentContext()
            resolver = local_context.ServiceManager.createInstanceWithContext(
                "com.sun.star.bridge.UnoUrlResolver", local_context
            )

            # Try both localhost and 127.0.0.1
            try:
                context = resolver.resolve(
                    "uno:socket,host=localhost,port=2002;urp;StarOffice.ComponentContext"
                )
            except NoConnectException:
                context = resolver.resolve(
                    "uno:socket,host=127.0.0.1,port=2002;urp;StarOffice.ComponentContext"
                )

            desktop = context.ServiceManager.createInstanceWithContext(
                "com.sun.star.frame.Desktop", context
            )
            return desktop
        except Exception as e:
            last_exception = e
            if attempt < retries and is_retryable_uno_error(e):
                logging.warning(
                    "Failed to get UNO desktop (attempt %s/%s): %s. Retrying in %.1fs.",
                    attempt,
                    retries,
                    e,
                    delay,
                )
                time.sleep(delay)
                continue
            break

    if last_exception is not None:
        print(f"Failed to get UNO desktop: {str(last_exception)}")
        print(traceback.format_exc())
    return None


def create_property_value(name, value):
    """Create a PropertyValue with given name and value."""
    prop = PropertyValue()
    prop.Name = name

    # Convert Python boolean to UNO boolean if needed
    if isinstance(value, bool):
        prop.Value = uno.Bool(value)
    else:
        prop.Value = value

    return prop


def open_document(file_path, read_only=False, retries=3, delay=0.5):
    print(f"Opening document: {file_path} (read_only: {read_only})")
    normalized_path = normalize_path(file_path)
    if not normalized_path.startswith(("file://", "http://", "https://", "ftp://")):
        if not os.path.exists(normalized_path):
            raise HelperError(f"Document not found: {normalized_path}")
        file_url = uno.systemPathToFileUrl(normalized_path)
    else:
        file_url = normalized_path

    last_exception = None
    for attempt in range(1, retries + 1):
        try:
            desktop = get_uno_desktop()
            if not desktop:
                raise HelperError("Failed to connect to LibreOffice desktop")

            props = [
                create_property_value("Hidden", True),
                create_property_value("ReadOnly", read_only),
            ]
            doc = desktop.loadComponentFromURL(file_url, "_blank", 0, tuple(props))
            if not doc:
                raise HelperError(f"Failed to load document: {file_path}")
            return doc, "Success"
        except Exception as e:
            last_exception = e
            print(f"Attempt {attempt} failed: {e}")
            if attempt >= retries or not is_retryable_uno_error(e):
                break
            logging.warning(
                "Retrying document open for %s after transient UNO error (attempt %s/%s): %s",
                file_path,
                attempt,
                retries,
                e,
            )
            time.sleep(delay)

    if last_exception:
        raise last_exception
    else:
        raise HelperError(
            f"Failed to load document after {retries} attempts: {file_path}"
        )
