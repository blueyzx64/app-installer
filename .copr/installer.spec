# F-63. Wraps the Tauri build. HUMAN: Copr builders have network by default; if disabled,
# vendor crates and node_modules into Source1 first.
Name:           installer
Version:        0.1.0
Release:        1%{?dist}
Summary:        Install apps by dragging them to Applications
# ASK O: license (placeholder)
License:        MIT
URL:            https://github.com/blueyzx64/installer
Source0:        installer-%{version}.tar.gz

BuildRequires:  cargo, rust, nodejs, npm, gcc, gcc-c++, pkgconf-pkg-config
BuildRequires:  webkit2gtk4.1-devel, gtk3-devel, dbus-devel, openssl-devel, librsvg2-devel, libsoup3-devel
BuildRequires:  desktop-file-utils, libappstream-glib
Requires:       PackageKit, webkit2gtk4.1

%description
Open an app file, drag it to Applications, done.

%prep
%autosetup -n installer-%{version}

%build
npm install --global pnpm@10.28.0 --prefix %{_builddir}/pnpm
export PATH=%{_builddir}/pnpm/bin:$PATH
pnpm install --frozen-lockfile
pnpm tauri build --no-bundle

%install
install -Dm755 src-tauri/target/release/installer %{buildroot}%{_bindir}/installer
sed -e 's/{{exec}}/installer/' -e 's/{{icon}}/io.github.blueyzx64.Installer/' \
  src-tauri/linux/installer.desktop > io.github.blueyzx64.Installer.desktop
install -Dm644 io.github.blueyzx64.Installer.desktop \
  %{buildroot}%{_datadir}/applications/io.github.blueyzx64.Installer.desktop
install -Dm644 src-tauri/linux/io.github.blueyzx64.Installer.metainfo.xml \
  %{buildroot}%{_datadir}/metainfo/io.github.blueyzx64.Installer.metainfo.xml
install -Dm644 src-tauri/icons/128x128.png \
  %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/io.github.blueyzx64.Installer.png

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/io.github.blueyzx64.Installer.desktop

%files
%{_bindir}/installer
%{_datadir}/applications/io.github.blueyzx64.Installer.desktop
%{_datadir}/metainfo/io.github.blueyzx64.Installer.metainfo.xml
%{_datadir}/icons/hicolor/128x128/apps/io.github.blueyzx64.Installer.png

%changelog
* Sun Oct 25 2026 Installer contributors - 0.1.0-1
- Initial package
