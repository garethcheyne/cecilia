#!/bin/bash -e
# Add Cecilia to Raspberry Pi OS Lite: the package (from the release build),
# network naming, and nothing else.
install -m 644 files/cecilia.deb "${ROOTFS_DIR}/tmp/cecilia.deb"
on_chroot <<CHROOT
apt-get install -y /tmp/cecilia.deb avahi-daemon
rm /tmp/cecilia.deb
systemctl enable cecilia.service
# Listen on the network out of the box; the first thing the guide asks for is
# a password.
sed -i 's/^#\?CECILIA_ADDR=.*/CECILIA_ADDR=0.0.0.0:7070/' /etc/default/cecilia
CHROOT
