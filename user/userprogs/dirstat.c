#include "types.h"
#include "user.h"


void main() {
    stat st;
    assert(mkdir("testdir") == 0, ", mkdir failed");
    int fd;
    assert((fd = open("testdir", 0)) >= 2, ", open failed");
    assert(fstat(fd, &st) == 0, ", chdir failed");
    assert(st.ino > 0, ", inode number invalid");
    assert(st.size == 32 * 2, ", size invalid");
}