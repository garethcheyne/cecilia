#!/bin/bash -e
# Start from the previous stage (Raspberry Pi OS Lite).
if [ ! -d "${ROOTFS_DIR}" ]; then
	copy_previous
fi
