#!/usr/bin/env python
import os
import unittest

from tempfile import mkstemp
from subprocess import Popen, PIPE
from random import randint


def generate_random_file(size):
    fd, fname = mkstemp()
    f = os.fdopen(fd, 'w')
    data = bytearray([randint(0, 255) for _ in range(size)])
    f.write(data)
    f.close()
    return fname


class NetcatClientTests(unittest.TestCase):

    def test_stdin_redirect_tcp_filesize_larger_buffer(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '12340'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/debug/nc', 'localhost', '12340'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())

    def test_stdin_redirect_tcp_zero_sized_file(self):
        fname = generate_random_file(0)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '12340'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/debug/nc', 'localhost', '12340'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())

    def test_stdin_redirect_tcp_small_filesize(self):
        fname = generate_random_file(1)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '12340'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/debug/nc', 'localhost', '12340'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())

    def test_stdin_redirect_udp4(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '-u', '-4', '-d',
                     '127.0.0.1', '12340'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/debug/nc', '-u', '-4', '-w',
                     '1', '127.0.0.1', '12340'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())

    def test_stdin_redirect_udp46(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '-u', '-d',
                     'localhost', '12340'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/debug/nc', '-v', '-u', '-w',
                     '1', '::1', '12340'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())

    def test_stdin_redirect_udp6(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '-u', '-d', '-6',
                     '12340'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['target/debug/nc', '-v', '-u', '-6', '-w',
                     '1', '::1', '12340'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())

    def test_stdin_pipe_tcp(self):
        outfd, outfilename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '12340'], stdout=PIPE)
        clt = Popen('echo bla | target/debug/nc localhost 12340', shell=True)
        out, _ = srv.communicate()
        self.assertEquals('bla\n', out)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())

if __name__ == "__main__":
    unittest.main(verbosity=2)
