#include "types.h"
#include "user.h"


void main() {
    assert(mkdir("testdir") == 0, ", mkdir failed");
    assert(chdir("testdir") == 0, ", chdir failed");
    assert(open("tmp.txt", 0) == -1, ", open should fail for non-existent file");
    int fd;
    assert((fd = open("tmp.txt", O_CREATE | O_RDWR)) > 2);
    assert(write(fd, "abcdefg", 7) == 7);
    assert(remove("tmp.txt") != -1, ", rm should not report failed on opened file");
    assert(seek(fd, 0)==0, ", seek failed");
    char buf[10];
    assert(read(fd, buf, 10) == 7,"expected to be able to read from removed opened file");
    close(fd);
    assert(open("tmp.txt", 0) == -1, ", open should fail for removed file");
}