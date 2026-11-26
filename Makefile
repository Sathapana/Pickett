CC=gcc
CFLAGS=-g -O3 -Wall
#CFLAGS=-O2 -Wall
EXEQ=spfit spcat calmrg dpfit dpcat calbak
EXEA=${EXEQ} iamcalc moiam stark termval sortn reassign sortegy iambak
# in-process test drivers (see pkselftest.c and test/run_golden.py --inprocess)
EXET=pkselftest_sp pkselftest_dp
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
# headers that every calpgm.h user depends on
HDR=calpgm.h slib.h pickett.h
default: ${EXEQ}
all: ${EXEA}
selftest: ${EXET}
install:
	-mv ${EXEQ} /usr/local/bin
clean:
	rm -f ${EXEA} ${EXET} *.o splib.a
.PHONY: default all selftest install clean
# SPFIT/SPCAT use spinv.c + spinit.c, DPFIT/DPCAT use dpi.c.
# calfitm.c/calcatm.c hold main(); the programs are calfit_run()/calcat_run().
dpfit: calfitm.o calfit.o subfit.o dpi.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
dpcat: calcatm.o calcat.o sortsub.o dpi.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
spfit: calfitm.o calfit.o subfit.o spinv.o spinit.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
spcat: calcatm.o calcat.o sortsub.o spinv.o spinit.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
pkselftest_sp: pkselftest.o calcat.o calfit.o subfit.o sortsub.o spinv.o spinit.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
pkselftest_dp: pkselftest.o calcat.o calfit.o subfit.o sortsub.o dpi.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
calmrg: calmrg.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
calbak: calbak.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
termval: termval.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
stark: stark.o splib.a ; $(CC) -o $@ $^ $(BLASLIB) -lm
moiam: moiam.o ftran.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
iamcalc: iamcalc.o ftran.o readopt.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
iambak: iambak.o readopt.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
reassign: reassign.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
sortegy: sortegy.o splib.a; $(CC) -o $@ $^ $(BLASLIB) -lm
sortn: sortn.o sortsub.o splib.a; $(CC) -o $@ $^

splib.a: ulib.o cnjj.o slibgcc.o catutil.o lsqfit.o $(LBLAS)
	ar r splib.a $^
	ranlib splib.a

calfit.o:calfit.c $(HDR)
calfitm.o:calfitm.c pickett.h
subfit.o:subfit.c $(HDR)
lsqfit.o:lsqfit.c lsqfit.h
calcat.o:calcat.c $(HDR)
calcatm.o:calcatm.c pickett.h
pkselftest.o:pkselftest.c pickett.h
sortsub.o: sortsub.c $(HDR)
calmrg.o:calmrg.c $(HDR)
termval.o:termval.c $(HDR)
stark.o:stark.c $(HDR)
iamcalc.o:iamcalc.c $(HDR) ftran.h readopt.h
iambak.o:iambak.c $(HDR) readopt.h
reassign.o:reassign.c $(HDR)
sortegy.o:sortegy.c $(HDR)
moiam.o:moiam.c $(HDR)
readopt.o:readopt.c readopt.h catutil.h
ulib.o:ulib.c $(HDR)
cnjj.o:cnjj.c cnjj.h
slibgcc.o:slibgcc.c slib.h pickett.h
spinv.o:spinv.c $(HDR) spinit.h
spinit.o:spinit.c $(HDR) spinit.h
dpi.o:dpi.c $(HDR)
ftran.o:ftran.c ftran.h
sortn.o: sortn.c
dblas.o: dblas.c cblas.h
calbak.o:calbak.c $(HDR)
