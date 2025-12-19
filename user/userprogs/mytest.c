
#include "sample.inc"
#include "types.h"
#include "user.h"
// ------------------- memory management -------------------
static size_t vac = 0;
static size_t head = 0x20000000; // initial brk
void* malloc(size_t size) {
    if(vac < size){
        int page = (size + 0x1000 -1) / 0x1000;
        brk (page * 0x1000);
        vac += page * 0x1000;
    }
    vac -= size;
    size_t ptr = head;
    head += size;
    return (void*)(ptr);
}

void free(void* p) {
    (void)p;  // no-op
}
/*
void* calloc(size_t nmemb, size_t size) {
    size_t total = nmemb * size;
    void* ptr = malloc(total);
    if (!ptr) return 0;
    char* c = (char*)ptr;
    for (size_t i = 0; i < total; i++) c[i] = 0;
    return ptr;
}
*/



void main() {
    void* p1 = malloc(1000);
    for(char* ptr = (char*)p1; ptr < (char*)p1 + 26;ptr++){
        *ptr = 'a' + (ptr - (char*)p1);
    }
    for(char* ptr = (char*)p1; ptr < (char*)p1 + 26;ptr++){
        printf("%c",*ptr);
    }
}