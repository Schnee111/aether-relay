import asyncio
import hmac
import hashlib
import json
import os
import signal
import socket
import sqlite3
import subprocess
import sys
import time
import urllib.request
import urllib.error
import aiohttp

BINARY_PATH = "/home/ubuntu/projects/aether-relay/target/release/aether-relay"

def get_free_port():
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.bind(('', 0))
        return s.getsockname()[1]

def compute_github_sig(secret: str, body: bytes) -> str:
    mac = hmac.new(secret.encode('utf-8'), body, hashlib.sha256)
    return f"sha256={mac.hexdigest()}"

print("Free port:", get_free_port())
