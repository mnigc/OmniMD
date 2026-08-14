<div align="center">

[English](README.md) | [简体中文](README.zh.md) | [한국어](README.ko.md) | [日本語](README.ja.md)

---

# OmniMD

### Anything to Markdown

PDF, Word, Excel, PowerPoint, EPUB, HTML 등 다양한 문서를 깔끔한 Markdown으로 변환하는 크로스 플랫폼 데스크톱 애플리케이션입니다.

**Tauri 2** + **Rust** + **React** 기반으로 구축되었으며, 핵심 변환 엔진은 [`anydoc`](https://crates.io/crates/anydoc)에서 제공됩니다.

</div>

---

## ✨ 주요 기능

- **20+ 포맷 지원** — PDF, DOC/DOCX, PPT/PPTX, XLS/XLSX, EPUB, CSV, TXT, HTML, ODT/ODS/ODP, RTF 등
- **배치 변환** — 여러 파일을 한 번에 드래그하여 동시 변환, 실시간 진행률 피드백
- **에셋 추출** — 문서 내 이미지 등 리소스를 자동으로 `assets/` 디렉토리에 저장
- **스마트 포맷 감지** — 매직 바이트를 통한 포맷 자동 감지, 확장자에 의존하지 않음
- **텍스트 패스스루** — 일반 텍스트, JSON, XML, HTML을 Markdown 블록으로 직접 변환
- **로컬 우선** — 모든 변환은 로컬에서 수행되며 파일이 업로드되지 않아 프라이버시 보장
- **네이티브 데스크톱 경험** — Tauri로 패키징된 가볍고 빠른 네이티브 앱

## 📸 스크린샷

> TODO: 스크린샷 추가

앱은 세 개의 메인 페이지로 구성됩니다:

| 페이지 | 용도 |
|--------|------|
| **Home** | 파일 탐색 및 빠른 액세스 |
| **Convert** | 단일 파일 변환, 출력 미리보기 |
| **Batch** | 배치 변환, 동시성 제어 및 진행률 추적 |

## 🚀 빠른 시작

### 사전 요구 사항

| 의존성 | 버전 | 비고 |
|--------|------|------|
| [Node.js](https://nodejs.org) | ≥ 18 (LTS) | 프론트엔드 런타임 |
| [pnpm](https://pnpm.io) | ≥ 8 | 패키지 매니저 |
| [Rust](https://www.rust-lang.org/tools/install) | stable | 백엔드 컴파일러 |
| MSVC Build Tools | — | Windows 필수 ("C++를 사용한 데스크톱 개발" 선택) |

### 설치 및 실행

```bash
# 1. 저장소 클론
git clone https://github.com/<your-org>/OmniMD.git
cd OmniMD

# 2. 프론트엔드 의존성 설치
cd frontend
pnpm install

# 3. Tauri CLI 추가 (최초 1회)
pnpm add -D @tauri-apps/cli

# 4. 개발 모드 시작 (Rust 백엔드 컴파일 + 프론트엔드 핫 리로드)
pnpm tauri dev
```

첫 실행 시 많은 Rust 의존성을 컴파일하므로 5~15분이 소요됩니다. 이후 증분 빌드는 빠릅니다. 준비가 완료되면 데스크톱 창이 자동으로 나타납니다.

## 📦 릴리스 빌드

```bash
cd frontend
pnpm tauri build
```

산출물은 `frontend/src-tauri/target/release/bundle/`에 위치합니다 (Windows에서 `.msi` / `.exe` 설치 파일).

## 🏗️ 프로젝트 구조

```
OmniMD/
├── src/                    # Rust 백엔드
│   ├── main.rs             # 진입점
│   ├── lib.rs              # Tauri 명령 등록 (프론트엔드 API)
│   ├── pipeline.rs         # 변환 파이프라인 (읽기 → 변환 → 쓰기)
│   ├── file_utils.rs       # 경로 헬퍼 / 포맷 화이트리스트
│   ├── converters/         # anydoc 변환기 구현
│   └── models/             # Document / Task / Asset 데이터 구조
├── frontend/               # React + TypeScript 프론트엔드
│   ├── src/
│   │   ├── App.tsx         # 앱 셸 및 내비게이션
│   │   ├── pages/          # Home / Convert / Batch 페이지
│   │   ├── api/            # Rust 백엔드 invoke 래퍼
│   │   ├── store/          # zustand 상태 관리
│   │   ├── components/     # 재사용 컴포넌트
│   │   └── types/          # 공유 타입 정의
│   └── vite.config.ts      # Vite 설정 (포트 1420)
├── tauri.conf.json         # Tauri 앱 설정
├── capabilities/           # Tauri 권한 설정
├── Cargo.toml              # Rust 의존성
└── tests/                  # 통합 테스트 + fixtures
```

## 🔌 프론트엔드–백엔드 통신

프론트엔드는 Tauri의 `invoke`를 통해 등록된 Rust 명령을 호출합니다. [`src/lib.rs`](src/lib.rs) 참조:

| 명령 | 매개변수 | 반환값 | 설명 |
|------|----------|--------|------|
| `convert_file` | `sourcePath`, `outputDir` | `ConversionResult` | 단일 파일 변환 |
| `convert_batch` | `sourcePaths`, `outputDir`, `concurrency` | `BatchResult` | 배치 동시 변환 |
| `get_supported_formats` | — | `string[]` | 지원 확장자 목록 |
| `get_converter_info` | — | `ConverterInfo` | 변환기 이름 및 포맷 |
| `preview_markdown` | `markdown` | `string` | 미리보기 엔드포인트 (현재 패스스루) |

변환 중 진행률은 Tauri 이벤트 `task-progress` / `task-status`를 통해 푸시되며, 프론트엔드는 이를 수신하여 실시간 UI를 업데이트합니다.

## 🧪 테스트

```bash
# Rust 단위 + 통합 테스트
cargo test

# 프론트엔드 타입 체크 + 빌드
cd frontend && pnpm build
```

테스트 fixtures는 [`tests/fixtures/`](tests/)에 있습니다.

## 🛠️ 기술 스택

| 계층 | 기술 |
|------|------|
| 데스크톱 프레임워크 | Tauri 2 |
| 백엔드 언어 | Rust 2021 edition |
| 변환 엔진 | [anydoc](https://crates.io/crates/anydoc) 0.1 |
| 비동기 런타임 | tokio |
| 프론트엔드 프레임워크 | React 18 + TypeScript 5 |
| 빌드 도구 | Vite 5 |
| 스타일링 | Tailwind CSS 3 |
| 상태 관리 | zustand |
| Markdown 렌더링 | react-markdown + remark-gfm |

## 📄 라이선스

TODO: LICENSE 파일 추가 (MIT 또는 Apache-2.0 권장).

## 🗺️ 로드맵

- [x] Phase 1 MVP — 단일 파일 / 배치 변환, anydoc 엔진 통합
- [ ] Settings 페이지 (출력 포맷, 동시성, OCR 토글)
- [ ] OCR 이미지 텍스트 인식 (모델 구조는 `src/models/ocr.rs`에 예약됨)
- [ ] 드래그 앤 드롭 폴더 재귀 변환
- [ ] 크로스 플랫폼 빌드 (macOS / Linux)

## 📖 추가 문서

상세한 개발, 디버깅, 패키징 지침은 **[개발·디버깅 가이드](docs/开发调试指南.md)** (중국어)를 참조하세요.

## 🤝 기여

Issue와 PR을 환영합니다. PR 제출 전 다음을 확인해 주세요:

```bash
cargo test                    # 백엔드 테스트 통과
cd frontend && pnpm build     # 프론트엔드 빌드 오류 없음
```

---

<div align="center">

Built with Tauri · Rust · React

</div>
