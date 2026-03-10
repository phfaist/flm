import socket


def find_available_port(host="localhost", base_port=8000, maxcount=64):
    """Find a port not in ues starting at given port"""
    count = 0
    while count <= maxcount:
        port = base_port + count
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            if s.connect_ex((host, port)) == 0:
                count += 1
                continue
            else:
                return port
    raise RuntimeError(f"Couldn't find a free port within {maxcount} of {base_port} on {host}")

