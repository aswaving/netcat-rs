#!/usr/bin/env python3
import os
import platform
import time
import unittest

from tempfile import mkstemp
from subprocess import Popen, PIPE
from random import randint

build = 'debug'

def generate_random_file(size):
    fd, fname = mkstemp()
    f = os.fdopen(fd, 'wb')
    data = bytes([randint(0, 255) for _ in range(size)])
    f.write(data)
    f.close()
    return fname


class NetcatClientTests(unittest.TestCase):

    def test_stdin_redirect_tcp_filesize_larger_buffer(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen([NC_PATH, '-l', '12340'], stdout=out_fd)
        time.sleep(0.1)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/' + build + '/nc', 'localhost', '12340'], stdin=infd)
        self.assertEqual(0, clt.wait())
        self.assertEqual(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEqual(0, diff.wait())

    def test_stdin_redirect_tcp_zero_sized_file(self):
        fname = generate_random_file(0)
        out_fd, out_filename = mkstemp()
        srv = Popen([NC_PATH, '-l', '12340'], stdout=out_fd)
        time.sleep(0.1)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/' + build + '/nc', 'localhost', '12340'], stdin=infd)
        self.assertEqual(0, clt.wait())
        self.assertEqual(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEqual(0, diff.wait())

    def test_stdin_redirect_tcp_small_filesize(self):
        fname = generate_random_file(1)
        out_fd, out_filename = mkstemp()
        srv = Popen([NC_PATH, '-l', '12340'], stdout=out_fd)
        time.sleep(0.1)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/' + build + '/nc', 'localhost', '12340'], stdin=infd)
        self.assertEqual(0, clt.wait())
        self.assertEqual(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEqual(0, diff.wait())

    def test_stdin_redirect_udp4(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen([NC_PATH, '-l', '-u', '-4', '-d',
                     '127.0.0.1', '12340'], stdout=out_fd)
        time.sleep(0.1)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/' + build + '/nc', '-u', '-4', '-w',
                     '1', '127.0.0.1', '12340'], stdin=infd)
        self.assertEqual(0, clt.wait())
        self.assertEqual(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEqual(0, diff.wait())

    def test_stdin_redirect_udp46(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen([NC_PATH, '-l', '-u', '-d',
                     '::1', '12340'], stdout=out_fd)
        time.sleep(0.1)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/' + build + '/nc', '-v', '-u', '-w',
                     '1', '::1', '12340'], stdin=infd)
        self.assertEqual(0, clt.wait())
        self.assertEqual(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEqual(0, diff.wait())

    def test_stdin_redirect_udp6(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen([NC_PATH, '-l', '-u', '-d', '-6',
                     '12340'], stdout=out_fd)
        time.sleep(0.1)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/' + build + '/nc', '-v', '-u', '-6', '-w',
                     '1', '::1', '12340'], stdin=infd)
        self.assertEqual(0, clt.wait())
        self.assertEqual(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEqual(0, diff.wait())

    def test_stdin_pipe_tcp(self):
        outfd, outfilename = mkstemp()
        srv = Popen([NC_PATH, '-l', '12340'], stdout=PIPE)
        time.sleep(0.1)
        clt = Popen('echo bla | target/' + build + '/nc localhost 12340', shell=True)
        out, _ = srv.communicate()
        self.assertEqual(b'bla\n', out)
        self.assertEqual(0, clt.wait())
        self.assertEqual(0, srv.wait())

if __name__ == "__main__":
    import os
    import sys

    if len(sys.argv) > 1:
        build = sys.argv[1]
        del sys.argv[1]
    
    print('testing ' + build + ' build')
    if not os.path.exists('target/' + build + '/nc'):
        print('Error: target executable `nc` does not exits in `target/' + build + '`')
        exit(1)

    if platform.system() == 'Linux':
        NC_PATH = '/bin/nc'
    else:
        NC_PATH = '/usr/bin/nc'

    unittest.main(verbosity=2)
