CC=gcc
CFLAGS=-g -O3 -Wall
#CFLAGS=-O2 -Wall
EXEQ=spfit spcat calmrg dpfit dpcat calbak
EXEA=${EXEQ} iamcalc moiam stark termval sortn reassign sortegy iambak
#next line for atlas blas
#BLASLIB=-lcblas -latlas
#next line for fortran blas and cblas wrappers
#BLASLIB=-lcblas -lblas
#next line for supplied routines
BLASLIB=
# use the bundled BLAS subset (dblas.c) only when no external BLAS is given
ifeq ($(strip $(BLASLIB)),)
LBLAS=dblas.o
endif
default: ${EXEQ}
all: ${EXEA}
install:
	-mv ${EXEQ} /usr/local/bin
clean:
	rm -f ${EXEA} *.o splib.a
.PHONY: default all install clean
dpfit: calfit.o subfit.o dpi.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
dpcat: calcat.o sortsub.o dpi.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
spfit: calfit.o subfit.o spinv.o spinit.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
spcat: calcat.o sortsub.o spinv.o spinit.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
calmrg: calmrg.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
calbak: calbak.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
termval: termval.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
stark: stark.o splib.a ; $(CC) -o $@ $^ $(BLASLIB) -lm
moiam: moiam.o ftran.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
iamcalc: iamcalc.o ftran.o readopt.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
iambak: iambak.o readopt.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
reassign: reassign.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
sortegy: sortegy.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
sortn: sortn.o sortsub.o; $(CC) -o $@ $^

splib.a: ulib.o cnjj.o slibgcc.o catutil.o lsqfit.o $(LBLAS)
	ar r splib.a $^
	ranlib splib.a

calfit.o:calfit.c calpgm.h
subfit.o:subfit.c calpgm.h
lsqfit.o:lsqfit.c lsqfit.h
calcat.o:calcat.c calpgm.h
sortsub.o: sortsub.c calpgm.h
calmrg.o:calmrg.c calpgm.h
termval.o:termval.c calpgm.h
stark.o:stark.c calpgm.h
iamcalc.o:iamcalc.c calpgm.h ftran.h readopt.h
iambak.o:iambak.c calpgm.h readopt.h
reassign.o:reassign.c calpgm.h
sortegy.o:sortegy.c calpgm.h
moiam.o:moiam.c calpgm.h
readopt.o:readopt.c readopt.h catutil.h
ulib.o:ulib.c calpgm.h
cnjj.o:cnjj.c cnjj.h
slibgcc.o:slibgcc.c calpgm.h
spinv.o:spinv.c calpgm.h spinit.h
spinit.o:spinit.c calpgm.h spinit.h
dpi.o:dpi.c calpgm.h
ftran.o:ftran.c ftran.h
sortn.o: sortn.c
dblas.o: dblas.c cblas.h
calbak.o:calbak.c calpgm.h
