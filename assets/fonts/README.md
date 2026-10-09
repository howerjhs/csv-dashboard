# 웹 빌드용 한글 폰트

- `DashboardKR-Regular.ttf` 는 **NanumGothic Regular**(© NHN Corporation, SIL Open Font License 1.1)의 서브셋입니다.
  - 원본: <https://github.com/google/fonts/tree/main/ofl/nanumgothic>
  - 포함 범위: ASCII/Latin-1, 일반 구두점·통화·기호, 한글 자모, 완성형 한글 11,172자, 전각 문자
  - 힌팅 제거, `subset.py`(fontTools)로 생성
- OFL의 예약 글꼴 이름(Reserved Font Name) "Nanum"/"NanumGothic" 조항에 따라 수정본은 `DashboardKR` 이라는 다른 이름을 사용합니다.
- 라이선스 전문: [OFL.txt](OFL.txt)

재생성: `python subset.py NanumGothic-Regular.ttf DashboardKR-Regular.ttf`
