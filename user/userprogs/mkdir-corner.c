#include "types.h"
#include "user.h"


void main() {
    assert(chdir(".") == 0, ", chdir failed");
    assert(chdir("..") == 0, ", chdir failed");
    assert(mkdir("tmpdir") == 0, ", mkdir failded");
    assert(mkdir("tmpdir") == -1, ", should not mkdir existing dir");
    int fd;
    assert((fd =open("tmpdir" ,O_CREATE | O_RDWR)) > 2, ", open failed");
    char buf[100];
    assert(read( fd ,buf,100 ) == -1, ", should not read from dir");
    assert(write( fd ,buf,100 ) == -1, ", should not write to dir");
}