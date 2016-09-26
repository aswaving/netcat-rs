import os
import unittest

from tempfile import mkstemp
from subprocess import Popen
from random import randint

def generate_random_file(size):
    fd, fname = mkstemp()
    f = os.fdopen(fd,'w')
    data = bytearray([randint(0,255) for _ in range(size)])
    f.write(data)
    f.close()
    return fname

class NetcatClientTests(unittest.TestCase):
    def test_stdin_redirect_tcp_filesize_larger_buffer(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '1234'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['nc', 'localhost', '1234'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())
    def test_stdin_redirect_tcp_zero_sized_file(self):
        fname = generate_random_file(0)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '1234'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['nc', 'localhost', '1234'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())
    def test_stdin_redirect_tcp_small_filesize(self):
        fname = generate_random_file(1)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-l', '1234'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['nc', 'localhost', '1234'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())
    def test_stdin_redirect_udp(self):
        fname = generate_random_file(32000)
        out_fd, out_filename = mkstemp()
        srv = Popen(['/usr/bin/nc', '-lu', '-w', '1', '1234'], stdout=out_fd)
        infd = os.open(fname, os.O_RDONLY)
        clt = Popen(['nc', '-vv', '-u', '-w', '1', 'localhost', '1234'], stdin=infd)
        self.assertEquals(0, clt.wait())
        self.assertEquals(0, srv.wait())
        diff = Popen(['diff', fname, out_filename])
        self.assertEquals(0, diff.wait())

if __name__ == "__main__":
    unittest.main()
