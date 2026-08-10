# RPM spec for unseen-servant, built from an already-built static musl
# binary (mirrors packaging/deb/build.sh's approach — the binary is built
# once, outside rpmbuild, and this spec only packages it). Build with
# packaging/rpm/build.sh, which stages the sources directory correctly.
#
# No sysusers.d macros here (unlike packaging/aur/sysusers.conf) — plain
# useradd/groupadd in %pre instead, since the systemd-rpm-macros sysusers
# helpers vary in availability/version across Fedora/RHEL/openSUSE, and
# plain useradd is universally supported. On erase, the user and
# /var/lib/usv are deliberately left alone (not deleted): RPM has no
# dpkg-style purge distinction, so "always keep the TOFU identity unless
# the operator explicitly removes it" (ADR 0003's posture, matched by
# packaging/deb/postrm's `remove` case) means simply never touching it
# in a scriptlet, not even on erase.

Name:           unseen-servant
Version:        %{_usv_version}
Release:        1%{?dist}
Summary:        security-first Gemini capsule server
License:        MIT
URL:            https://forgejo.wanderingmonster.dev/WanderingMonster/unseen-servant

Source0:        usv
Source1:        usv.service
Source2:        LICENSE
Source3:        README.md

BuildArch:      x86_64
Requires(pre):  shadow-utils
%{?systemd_requires}
BuildRequires:  systemd-rpm-macros

# Prebuilt static musl binary — nothing to compile, and no debuginfo to
# extract from it (find-debuginfo.sh chokes on a foreign static binary
# with no matching build-id/source mapping in this tree).
%global debug_package %{nil}
%global __os_install_post %{nil}

%description
Unseen Servant (usv) publishes one content tree to Geminispace (gemtext,
port 1965) and, optionally, to the web as statically rendered classless
HTML. TOFU-native identity, Titan uploads, and a terminal setup wizard.
.
Statically linked (musl) — no runtime dependencies beyond a Linux kernel.
Pre-release software; see docs/BUILD-PLAN.md on the project's own repo
for the current phase.

%prep
# nothing to unpack — Source0 is the finished binary

%build
# nothing to compile — see the header comment

%install
install -Dm755 %{SOURCE0} %{buildroot}%{_bindir}/usv
install -Dm644 %{SOURCE1} %{buildroot}%{_unitdir}/usv.service
install -Dm644 %{SOURCE2} %{buildroot}%{_licensedir}/%{name}/LICENSE
install -Dm644 %{SOURCE3} %{buildroot}%{_docdir}/%{name}/README.md

%pre
getent group usv >/dev/null || groupadd -r usv
getent passwd usv >/dev/null || useradd -r -g usv -d /var/lib/usv \
    -s /sbin/nologin -c "Unseen Servant" usv
mkdir -p /var/lib/usv
chown usv:usv /var/lib/usv
exit 0

%post
%systemd_post usv.service

%preun
%systemd_preun usv.service

%postun
%systemd_postun_with_restart usv.service

%files
%{_bindir}/usv
%{_unitdir}/usv.service
%license %{_licensedir}/%{name}/LICENSE
%doc %{_docdir}/%{name}/README.md

%changelog
* Mon Aug 10 2026 Wandering Monster <most+claude@alba.win> - 0.1.0-1
- Initial packaging (pre-release)
