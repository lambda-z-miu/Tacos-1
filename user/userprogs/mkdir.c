#include "types.h"
#include "user.h"


void main() {
    assert(mkdir("testdir") == 0, ", mkdir failed");
    assert(chdir("testdir") == 0, ", chdir failed");
    assert(open("tmp.txt", 0) == -1, ", open should fail for non-existent file");
    int fd;
    assert((fd = open("tmp.txt", O_CREATE | O_WRONLY)) > 2);
    assert(write(fd, "abcdefg", 7) == 7);
    close(fd);
    assert(chdir("..") == 0, ", chdir failed");
    assert((fd = open("tmp.txt", 0)) == -1, ", open should fail");
    assert(chdir("testdir") == 0, ", chdir failed");
    char buf[10];
    assert((fd = open("tmp.txt", O_RDONLY)) > 2);
    assert(read(fd, buf, 10) == 7);
    printf("Read from file: %s\n", buf);
}