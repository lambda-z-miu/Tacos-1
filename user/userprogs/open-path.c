/** Opens a file to read and/or write. */

#include "sample.inc"
#include "user.h"

void main() {
    int fd;
    char buf[256];
    memset(buf, 0, sizeof buf);

    assert(mkdir("testdir") == 0, ", mkdir failed");
    assert(chdir("testdir") == 0, ", chdir failed");
    assert((fd = open("file1", O_CREATE | O_RDWR)) >= 2, ", open failed");
    close(fd);
    assert(chdir("..") == 0, ", chdir failed");

    assert((fd = open("testdir/file1", O_RDWR)) >= 2, ", open failed");
    const char* msg = "Hello, World!\n";
    assert(write(fd, msg, strlen(msg)) == (int)strlen(msg), ", write failed");
    assert(seek(fd, 0) == 0, ", seek failed");
    assert(read(fd, buf, sizeof buf) == (int)strlen(msg), ", read failed");
    assert(strcmp(buf, msg) == 0, ", content mismatch");
    assert(close(fd) == 0, ", close failed");  

    assert(remove("testdir/file1")==0, ", remove failed");
    // assert(remove("testdir")==0, ", remove failed");
    assert(open("testdir/file1", O_RDWR) < 0, ", open should have failed");
}
