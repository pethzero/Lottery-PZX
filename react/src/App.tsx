import { useState } from "react";

const cards = [
  {
    label: "Card 2",
    title: "ตรวจผลย้อนหลัง",
    description: "ค้นหาผลรางวัลย้อนหลังตามงวดที่ต้องการได้อย่างรวดเร็ว",
    button: "ตรวจสอบ",
  },
  {
    label: "Card 3",
    title: "ตั้งค่าการแจ้งเตือน",
    description: "รับการแจ้งเตือนเมื่อมีผลรางวัลใหม่หรือเลขที่สนใจ",
    button: "ตั้งค่า",
  },
];

function App() {
  const [lotteryResult, setLotteryResult] = useState<Record<string, unknown> | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [importMessage, setImportMessage] = useState<string | null>(null);

  const loadLatestLottery = async () => {
    setLoading(true);
    setError(null);
    setImportMessage(null);
    setLotteryResult(null);

    try {
      const response = await fetch("/api/last-lottery");
      if (!response.ok) {
        throw new Error(`Server returned ${response.status}`);
      }

      const json = await response.json();
      console.log("GLO latest lottery response:", json);
      setLotteryResult(json);
    } catch (err) {
      console.error("Failed to load latest lottery:", err);
      setError(err instanceof Error ? err.message : "เกิดข้อผิดพลาดไม่ทราบ");
    } finally {
      setLoading(false);
    }
  };

  const importLottery = async () => {
    if (!lotteryResult) return;

    setImporting(true);
    setImportMessage(null);

    try {
      console.log("Importing lottery source:", lotteryResult.source);

      const response = await fetch("/api/import-lottery", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(lotteryResult.source),
      });

      if (!response.ok) {
        throw new Error(`Server returned ${response.status}`);
      }

      const result = await response.json();
      console.log("Import response:", result);
      setImportMessage(result.message || "Import completed");
    } catch (err) {
      console.error("Import lottery failed:", err);
      setImportMessage(err instanceof Error ? err.message : "เกิดข้อผิดพลาดในการนำเข้าข้อมูล");
    } finally {
      setImporting(false);
    }
  };

  return (
    <div className="app-shell">
      <div className="page-header">
        <h1>Lottery PZX</h1>
        <p>กดปุ่มในการ์ดแรกเพื่อเรียกข้อมูลล่าสุดจาก GLO API แล้วแสดงผล</p>
      </div>

      <div className="cards-grid">
        <div className="card small-card">
          <div className="card-label">Card 1</div>
          <h2>API ดึงข้อมูลวันล่าสุดของ GLO</h2>
          <p>กดปุ่มด้านล่างเพื่อเรียกผลสลากกินแบ่งรัฐบาลล่าสุดจาก API ของ GLO</p>
          <button type="button" onClick={loadLatestLottery} disabled={loading}>
            {loading ? "กำลังโหลด..." : "ดูรายละเอียด"}
          </button>
          {error && <div className="error">Error: {error}</div>}

          {lotteryResult && (
            <>
              <button type="button" className="secondary-button" onClick={importLottery} disabled={importing}>
                {importing ? "นำเข้าฐานข้อมูล..." : "นำเข้าฐานข้อมูล"}
              </button>
              {importMessage && <div className="import-message">{importMessage}</div>}
              <div className="result">
                <pre>{JSON.stringify(lotteryResult, null, 2)}</pre>
              </div>
            </>
          )}
        </div>

        {/* {cards.map((card) => (
          <div key={card.title} className="card small-card">
            <div className="card-label">{card.label}</div>
            <h2>{card.title}</h2>
            <p>{card.description}</p>
            <button type="button">{card.button}</button>
          </div>
        ))} */}
      </div>
    </div>
  );
}

export default App;
