# Builds the CALPGM programs from src/ into bin/ (objects and splib.a in build/).
#   make            spfit spcat calmrg dpfit dpcat calbak
#   make all        every program
#   make selftest   in-process test drivers (see src/pkselftest.c, test/run_golden.py --inprocess)
#   make spcat      one program (any name below works the same way)
#   make clean      remove build/ and bin/
CC=gcc
CFLAGS=-g -O3 -Wall
#CFLAGS=-O2 -Wall
SRC=src
BUILD=build
BIN=bin
EXEQ=spfit spcat calmrg dpfit dpcat calbak
EXEA=${EXEQ} iamcalc moiam stark termval sortn reassign sortegy iambak cnvir hundscvt
EXET=pkselftest_sp pkselftest_dp
#next line for atlas blas
#BLASLIB=-lcblas -latlas
#next line for fortran blas and cblas wrappers
#BLASLIB=-lcblas -lblas
#next line for supplied routines
BLASLIB=
# use the bundled BLAS subset (dblas.c) only when no external BLAS is given
ifeq ($(strip $(BLASLIB)),)
LBLAS=dblas
endif
ifeq ($(OS),Windows_NT)
EXE=.exe
# pktmp.c uses threads for the partition-function table: POSIX needs pthread
THREADLIB=
else
EXE=
THREADLIB=-lpthread
endif

# Objects of each program, besides build/splib.a.
# SPFIT/SPCAT use spinv.c + spinit.c, DPFIT/DPCAT use dpi.c.
# calfitm.c/calcatm.c hold main(); the programs are calfit_run()/calcat_run().
OBJ_spfit=calfitm calfit subfit spinv spinit
OBJ_spcat=calcatm calcat sortsub spinv spinit
OBJ_dpfit=calfitm calfit subfit dpi
OBJ_dpcat=calcatm calcat sortsub dpi
OBJ_pkselftest_sp=pkselftest calcat calfit subfit sortsub spinv spinit
OBJ_pkselftest_dp=pkselftest calcat calfit subfit sortsub dpi
OBJ_calmrg=calmrg
OBJ_calbak=calbak
OBJ_termval=termval
OBJ_stark=stark
OBJ_moiam=moiam ftran
OBJ_iamcalc=iamcalc ftran readopt
OBJ_iambak=iambak readopt
OBJ_reassign=reassign
OBJ_sortegy=sortegy
OBJ_cnvir=cnvir
OBJ_hundscvt=hundscvt
OBJ_sortn=sortn sortsub
# the shared library
OBJ_splib=ulib cnjj slibgcc pktmp catutil lsqfit $(LBLAS)

objs=$(addprefix $(BUILD)/,$(addsuffix .o,$(1)))

default: ${EXEQ}
all: ${EXEA}
selftest: ${EXET}
install: ${EXEQ}
	-mv $(addprefix $(BIN)/,$(addsuffix $(EXE),${EXEQ})) /usr/local/bin
clean:
	rm -rf $(BUILD) $(BIN)
.PHONY: default all selftest install clean

# bin/<name> from its objects; `make <name>` is an alias for it
define PROGRAM
$(BIN)/$(1)$(EXE): $$(call objs,$$(OBJ_$(1))) $(BUILD)/splib.a | $(BIN)
	$$(CC) -o $$@ $$^ $$(BLASLIB) -lm $$(THREADLIB)
$(1): $(BIN)/$(1)$(EXE)
.PHONY: $(1)
endef
$(foreach p,$(EXEA) $(EXET),$(eval $(call PROGRAM,$(p))))

$(BUILD)/splib.a: $(call objs,$(OBJ_splib))
	rm -f $@
	ar r $@ $^
	ranlib $@

# every object depends on every header: simple, and never stale
$(BUILD)/%.o: $(SRC)/%.c $(wildcard $(SRC)/*.h) | $(BUILD)
	$(CC) $(CFLAGS) -I$(SRC) -c $< -o $@

$(BUILD) $(BIN):
	mkdir -p $@
