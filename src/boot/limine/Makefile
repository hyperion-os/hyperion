LIM_GIT   := https://github.com/limine-bootloader/limine.git
LIM_DIR   := ${TARGET_DIR}/limine
LIM_FILES := ${LIM_DIR}/limine-cd-efi.bin ${LIM_DIR}/limine-cd.bin ${LIM_DIR}/limine.sys
LIM_CFG   := cfg/limine.cfg

XORRISO_FLAGS ?=
XORRISO_FLAGS += -as mkisofs
XORRISO_FLAGS += -b limine-cd.bin
XORRISO_FLAGS += -no-emul-boot 
XORRISO_FLAGS += -boot-load-size 4
XORRISO_FLAGS += -boot-info-table
XORRISO_FLAGS += --efi-boot limine-cd-efi.bin
XORRISO_FLAGS += -efi-boot-part
XORRISO_FLAGS += --efi-boot-image
XORRISO_FLAGS += --protective-msdos-label

# clone limine
${LIM_FILES}:
	@echo -e "\n\033[32m--[[ cloning Limine ]]--\033[0m"
	git clone "${LIM_GIT}" --depth=1 --branch v3.0-branch-binary ${LIM_DIR}
	cd ${LIM_DIR}; make

# create ISO
${HYPERION}: ${KERNEL} ${LIM_CFG} ${LIM_FILES} Makefile
	@echo -e "\n\033[32m--[[ creating Hyperion ISO ]]--\033[0m"
	mkdir -p ${ISO_DIR}
	cp ${LIM_CFG} ${LIM_FILES} ${ISO_DIR}
	cp ${KERNEL} ${ISO_DIR}/hyperion

	${XORRISO} ${XORRISO_FLAGS} ${ISO_DIR} -o ${HYPERION}

	${LIM_DIR}/limine-deploy ${HYPERION}

# create testing ISO
${HYPERION_TESTING}: ${KERNEL_TESTING} ${LIM_CFG} ${LIM_FILES} Makefile
	@echo -e "\n\033[32m--[[ creating Hyperion-Testing ISO ]]--\033[0m"
	mkdir -p ${ISO_TESTING_DIR}
	cp ${LIM_CFG} ${LIM_FILES} ${ISO_TESTING_DIR}
	cp ${KERNEL_TESTING} ${ISO_TESTING_DIR}/hyperion

	${XORRISO} ${XORRISO_FLAGS} ${ISO_TESTING_DIR} -o ${HYPERION_TESTING}

	${LIM_DIR}/limine-deploy ${HYPERION_TESTING}
