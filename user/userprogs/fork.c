#include "user.h"

void main() {
    int a = 100;
    if(fork()){
        a += 100;
        printf("parent here %d \n",a);
        exit(0);
    }
    else{
        a += 200;
        printf("child here %d \n",a);
        exit(0);
    }
}
