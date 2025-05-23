#!/usr/bin/env python3
import asyncio
import socket
import sys
import datetime
import argparse
from typing import List, Tuple
import logging
import os
import subprocess

class IPv6Tester:
    DEFAULT_PORT = 8080
    DEFAULT_IPV6_ADDRESS = "::1"
    MAX_CLIENTS = 10
    DATE_FORMAT = "%Y-%m-%d %H:%M:%S"

    def __init__(self):
        self.logger = logging.getLogger(__name__)
        self.logger.setLevel(logging.INFO)
        handler = logging.StreamHandler()
        formatter = logging.Formatter('%(message)s')
        handler.setFormatter(formatter)
        self.logger.addHandler(handler)

    def print_usage(self) -> None:
        """Print usage information and available IPv6 addresses."""
        self.logger.info("Usage: python ipv6_tester.py <server|client> [ipv6_address] [port]")
        self.logger.info("  server|client    - Required. Run as server or client")
        self.logger.info("  ipv6_address     - Optional. IPv6 address (default: ::1)")
        self.logger.info("  port             - Optional. Port number (default: 8080)")
        
        self.logger.info("\nAvailable IPv6 addresses on this host:")
        self.print_available_ipv6_addresses()
        
        self.logger.info("\nPython IPv6 related properties:")
        self.logger.info(f"  socket.AF_INET6: {socket.AF_INET6}")
        self.logger.info(f"  socket.has_ipv6: {socket.has_ipv6}")
        self.logger.info(f"  environment variable IPV6_V6ONLY: {os.environ.get('IPV6_V6ONLY', 'not set')}")
        
        # Check system IPv6 configuration
        try:
            with open('/proc/sys/net/ipv6/conf/all/disable_ipv6', 'r') as f:
                ipv6_disabled = f.read().strip() == '1'
            self.logger.info(f"  System IPv6 enabled: {not ipv6_disabled}")
        except FileNotFoundError:
            # Not on Linux, try macOS
            try:
                import subprocess
                result = subprocess.run(['sysctl', 'net.inet6.ip6.forwarding'], 
                                     capture_output=True, text=True)
                self.logger.info(f"  System IPv6 forwarding: {result.stdout.strip()}")
            except FileNotFoundError:
                self.logger.info("  System IPv6 status: Unable to determine")
        
        self.logger.info("\nExamples:")
        self.logger.info("  python ipv6_tester.py server")
        self.logger.info("  python ipv6_tester.py server 2001:db8:1234:5678::1")
        self.logger.info("  python ipv6_tester.py client 2001:db8:1234:5678::1 8888")

    def print_available_ipv6_addresses(self) -> None:
        """Print all available IPv6 addresses on the system."""
        try:
            ipv6_addresses = [
                f"  {interface[3]}: {interface[4][0]}"
                for interface in socket.getaddrinfo(host=socket.gethostname(), port=None, family=socket.AF_INET6, proto=socket.IPPROTO_TCP)
            ]
            for addr in ipv6_addresses:
                self.logger.info(addr)
        except Exception as e:
            self.logger.error(f"Error getting network interfaces: {e}")

    def log_socket_properties(self, writer: asyncio.StreamWriter, context: str) -> None:
        """Log IPv6 properties of a socket from a stream writer."""
        try:
            sock = writer.get_extra_info('socket')
            self.logger.info(f"\nSocket properties for {context}:")
            self.logger.info(f"  Socket family: {sock.family}")
            self.logger.info(f"  Socket type: {sock.type}")
            self.logger.info(f"  Socket protocol: {sock.proto}")
            self.logger.info(f"  Socket IPv6 only: {sock.getsockopt(socket.IPPROTO_IPV6, socket.IPV6_V6ONLY)}")
        except Exception as e:
            self.logger.error(f"Could not get socket properties for {context}: {e}")

    async def handle_client(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter, server_address: str) -> None:
        """Handle individual client connections."""
        client_address = writer.get_extra_info('peername')[0]
        self.logger.info(f"Client connected from: [{client_address}]")
        self.log_socket_properties(writer, f"client connection from [{client_address}]")

        try:
            while True:
                # Read client message
                data = await reader.readline()
                if not data:
                    self.logger.info(f"Client disconnected: [{client_address}]")
                    break

                message = data.decode().strip()
                self.logger.info(f"Received from client [{client_address}]: {message}")

                # Send response with timestamp
                timestamp = datetime.datetime.now().strftime(self.DATE_FORMAT)
                response = f"Server received your message at {timestamp} at address {server_address}\n"
                writer.write(response.encode())
                await writer.drain()

                # Add a delay of 1 second
                await asyncio.sleep(1)

        except Exception as e:
            self.logger.error(f"Error handling client [{client_address}]: {e}")
        finally:
            writer.close()
            await writer.wait_closed()

    async def run_server(self, ipv6_address: str, port: int) -> None:
        """Run the IPv6 server."""
        try:
            server = await asyncio.start_server(
                lambda r, w: self.handle_client(r, w, ipv6_address),
                ipv6_address,
                port,
                family=socket.AF_INET6
            )
            self.logger.info(f"IPv6 Server started on [{ipv6_address}]:{port}")
            self.logger.info(f"Maximum number of simultaneous clients: {self.MAX_CLIENTS}")

            async with server:
                await server.serve_forever()
        except Exception as e:
            self.logger.error(f"Server error: {e}")

    async def run_client(self, ipv6_address: str, port: int) -> None:
        """Run the IPv6 client."""
        try:
            reader, writer = await asyncio.open_connection(
                ipv6_address,
                port,
                family=socket.AF_INET6
            )
            self.logger.info(f"Connected to server at [{ipv6_address}]:{port}")
            self.log_socket_properties(writer, f"client connection to [{ipv6_address}]:{port}")

            try:
                for i in range(20):
                    # Send message to server
                    timestamp = datetime.datetime.now().strftime(self.DATE_FORMAT)
                    message = f"Hello from Python IPv6 client at {timestamp}\n"
                    writer.write(message.encode())
                    await writer.drain()
                    self.logger.info(f"Sent to server: {message.strip()}")

                    # Read server response
                    response = await reader.readline()
                    self.logger.info(f"Server response: {response.decode().strip()}")

                    # Wait 1 second before next iteration
                    if i < 19:
                        await asyncio.sleep(1)

            finally:
                writer.close()
                await writer.wait_closed()

        except Exception as e:
            self.logger.error(f"Client error: {e}")

    def main(self) -> None:
        """Main entry point for the IPv6 tester."""
        if len(sys.argv) < 2:
            self.print_usage()
            sys.exit(1)

        mode = sys.argv[1]
        ipv6_address = sys.argv[2] if len(sys.argv) > 2 else self.DEFAULT_IPV6_ADDRESS
        port = int(sys.argv[3]) if len(sys.argv) > 3 else self.DEFAULT_PORT

        if mode not in ['server', 'client']:
            self.print_usage()
            sys.exit(1)

        try:
            if mode == 'server':
                asyncio.run(self.run_server(ipv6_address, port))
            else:
                asyncio.run(self.run_client(ipv6_address, port))
        except KeyboardInterrupt:
            self.logger.info("\nShutting down...")
        except Exception as e:
            self.logger.error(f"Error: {e}")
            sys.exit(1)

if __name__ == "__main__":
    tester = IPv6Tester()
    tester.main() 