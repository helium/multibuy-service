#!/usr/bin/env bash

set -euo pipefail

cd $GITHUB_WORKSPACE

if [ -z "$GITHUB_REF" ]; then
    git config --global --add safe.directory "$GITHUB_WORKSPACE"
    VERSION=$(git describe)
else
    VERSION=$(echo "$GITHUB_REF" | sed 's|refs/tags/||')
fi


write_unit_template()
{

    cat << -EOF >"/tmp/multi_buy_service.service"
[Unit]
Description=multi_buy_service
After=network.target
StartLimitInterval=60
StartLimitBurst=3

[Service]
Type=simple
ExecStart=/opt/multi_buy_service/bin/multi_buy_service -c /opt/multi_buy_service/etc/settings.toml
User=helium
PIDFile=/var/run/multi_buy_service
Restart=always
RestartSec=15
WorkingDirectory=/opt/multi_buy_service

### Remove default limits from a few important places:
LimitNOFILE=infinity
LimitNPROC=infinity
TasksMax=infinity

[Install]
WantedBy=multi-user.target
-EOF
}

write_prepost_template()
{
    cat << -EOF >"/tmp/multi_buy_service-preinst"
# add system user for file ownership and systemd user, if not exists
useradd --system --home-dir /opt/helium --create-home helium || true
-EOF

    cat << -EOF >"/tmp/multi_buy_service-postinst"
# add to /usr/local/bin so it appears in path
ln -s /opt/multi_buy_service/bin/multi_buy_service /usr/local/bin/multi_buy_service || true
-EOF

    cat << -EOF >"/tmp/multi_buy_service-postrm"
rm -f /usr/local/bin/multi_buy_service
-EOF
}

run_fpm()
{
    local VERSION=$1

    # XXX HACK fpm won't let us mark a config file unless
    # it exists at the specified path
    mkdir -p /opt/multi_buy_service/etc
    touch /opt/multi_buy_service/etc/settings.toml

    fpm -n downlink-service \
        -v "${VERSION}" \
        -s dir \
        -t deb \
        --deb-systemd "/tmp/multi_buy_service.service" \
        --before-install "/tmp/multi_buy_service-preinst" \
        --after-install "/tmp/multi_buy_service-postinst" \
        --after-remove "/tmp/multi_buy_service-postrm" \
        --deb-no-default-config-files \
        --deb-systemd-enable \
        --deb-systemd-auto-start \
        --deb-systemd-restart-after-upgrade \
        --deb-user helium \
        --deb-group helium \
        --config-files /opt/multi_buy_service/etc/settings.toml \
        target/release/multi_buy_service=/opt/multi_buy_service/bin/multi_buy_service \
        pkg/settings-template.toml=/opt/multi_buy_service/etc/settings-example.toml

    # copy deb to /tmp for upload later
    cp *.deb /tmp

}

# install fpm
sudo apt update
sudo apt install --yes ruby
sudo gem install fpm -v 1.15.1 # current as of 2023-02-21

write_unit_template
write_prepost_template
run_fpm $VERSION

for deb in /tmp/*.deb
do
    echo "uploading $deb"
    curl -u "${PACKAGECLOUD_API_KEY}:" \
         -F "package[distro_version_id]=210" \
         -F "package[package_file]=@$deb" \
         https://packagecloud.io/api/v1/repos/helium/packet_router/packages.json
done
