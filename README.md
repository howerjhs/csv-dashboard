# CSV 대시보드

CSV 파일을 끌어다 놓으면 추이·분류별 집계·비중·분포 차트와 요약 통계를 만들어 주는 대시보드입니다.
Rust [eframe/egui](https://github.com/emilk/egui) 0.31 + egui_plot 0.31로 작성되었고, 같은 코드로

- **Windows 데스크톱 앱** (`CSV_Dashboard.exe`)
- **웹앱** (WebAssembly, Netlify 등 정적 호스팅)

두 가지로 빌드됩니다. UTF-8 / CP949(엑셀 한글) CSV를 자동 인식하며, 웹 버전에서도 파일은 브라우저 안에서만 처리되고 서버로 전송되지 않습니다.

## 구조

| 경로 | 설명 |
| --- | --- |
| `src/main.rs` | 앱 전체. `#[cfg(target_arch = "wasm32")]`로 웹/데스크톱 진입점·폰트·파일 열기를 분기 |
| `index.html` | Trunk 웹 빌드용 페이지(캔버스, 로딩 표시) |
| `assets/fonts/` | 웹용 한글 폰트(나눔고딕 서브셋, SIL OFL 1.1) — [assets/fonts/README.md](assets/fonts/README.md) |
| `build.bat` | 데스크톱 릴리스 빌드 후 `CSV_Dashboard.exe`를 이 폴더로 복사 |
| `netlify.toml` | Netlify 설정(빌드는 GitHub Actions가 하므로 Netlify 쪽 빌드 없음) |
| `.github/workflows/deploy.yml` | CI: 웹 빌드 → Netlify 배포, Windows exe 빌드 → 태그 시 Release 첨부 |

| | 데스크톱 | 웹 |
| --- | --- | --- |
| 한글 폰트 | Windows 시스템 폰트(맑은 고딕 등) | 임베드된 `DashboardKR-Regular.ttf` |
| 파일 열기 버튼 | `rfd::FileDialog`(동기) | `rfd::AsyncFileDialog` + `spawn_local`, 결과는 `Arc<Mutex<…>>`로 다음 프레임에 전달 |
| 드래그 앤 드롭 | 파일 경로 | 파일 바이트 |

## 로컬 빌드

### 데스크톱 (Windows, GNU 툴체인)

준비: `rustup` stable-x86_64-pc-windows-gnu, WinLibs MinGW(`winget install BrechtSanders.WinLibs.POSIX.UCRT`).

```bat
build.bat
```

MinGW `dlltool`이 한글 경로를 처리하지 못하므로 `build.bat`은 `CARGO_TARGET_DIR`을
`%USERPROFILE%\.cargo\targets\csv_dashboard`로 지정해 영문 경로에서 빌드합니다(환경변수를 미리 지정하면 그 값을 사용).
`.cargo/config.toml`은 같은 설정을 담은 **로컬 전용** 파일로 `.gitignore` 대상입니다.

### 웹 (WASM)

```bash
rustup target add wasm32-unknown-unknown
```

Trunk 설치는 `cargo install trunk --locked`가 기본이지만, 최신 WinLibs GCC(16.x)에서는 의존성 `libdeflate-sys`가
빌드되지 않습니다. 이 경우 [Trunk 릴리스](https://github.com/trunk-rs/trunk/releases)의
`trunk-x86_64-pc-windows-msvc.zip`을 받아 `trunk.exe`를 `%USERPROFILE%\.cargo\bin`에 두면 됩니다.

```bash
trunk serve --release
```

브라우저에서 <http://127.0.0.1:8080> 을 열고 `sample_sales.csv`를 끌어다 놓아 보세요. 배포용 빌드:

```bash
trunk build --release --public-url ./
```

`dist/`에 `index.html`, `*.js`, `*_bg.wasm`(약 4.6 MB, 폰트 1.3 MB 포함)이 생성됩니다.
`index.html`의 `data-wasm-opt-params`는 최신 rustc가 기본으로 쓰는 wasm 기능(bulk memory 등)을 wasm-opt에 알려 주기 위한 것이므로 지우지 마세요.

## GitHub에 올리기

1. GitHub에서 빈 저장소를 만듭니다(README/.gitignore 추가하지 않음).
2. 이 폴더에서:

   ```bash
   git remote add origin https://github.com/<사용자>/<저장소>.git
   git push -u origin main
   ```

   (`git init`과 첫 커밋은 이미 되어 있습니다. `dist/`, `*.exe`, `.cargo/`는 `.gitignore`로 제외됩니다.)

## Netlify 배포 (GitHub Actions)

워크플로 `.github/workflows/deploy.yml`이 `main`에 push될 때마다 웹을 빌드해 Netlify에 올립니다.
Netlify 자체 빌드 서버에는 Rust가 없으므로, Netlify는 **결과물만 받는** 사이트로 만듭니다.

1. **사이트 만들기** — Netlify에서 *Add new site → Deploy manually*를 선택하고, 로컬에서 만든 `dist` 폴더를
   한 번 끌어다 놓아 빈 사이트를 만듭니다(또는 `npx netlify-cli sites:create`).
   Git 저장소 연결(Import from Git)은 하지 마세요. 연결하면 Netlify가 직접 빌드를 시도합니다.
2. **Site ID 확인** — *Site configuration → General → Site details*의 **Site ID**(=API ID)를 복사합니다.
3. **액세스 토큰 발급** — *User settings → Applications → Personal access tokens → New access token*.
4. **GitHub Secrets 등록** — 저장소 *Settings → Secrets and variables → Actions → New repository secret*:
   - `NETLIFY_AUTH_TOKEN` : 3에서 발급한 토큰
   - `NETLIFY_SITE_ID` : 2의 Site ID
5. `main`에 push하면 *Actions* 탭에서 `Build & Deploy`가 실행되고 Netlify 사이트가 갱신됩니다.
   secrets가 없으면 빌드만 하고 배포 단계는 경고와 함께 건너뜁니다. PR에서는 빌드만 확인합니다.

## Windows exe 릴리스

같은 워크플로의 `windows` 잡이 매번 exe를 빌드해 Actions 아티팩트로 올립니다.
`v`로 시작하는 태그를 push하면 GitHub Release에 `CSV_Dashboard.exe`가 첨부됩니다.

```bash
git tag v0.1.0
git push origin v0.1.0
```

## 라이선스

- 웹 빌드에 포함된 폰트: NanumGothic(© NHN Corporation) 서브셋, SIL Open Font License 1.1 — [assets/fonts/OFL.txt](assets/fonts/OFL.txt)
