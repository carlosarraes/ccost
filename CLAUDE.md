# ccost Development Notes

## Deduplication Accuracy Analysis (TASK-044-CRITICAL)

### Background
ccost was showing higher costs (~$1250) compared to competitor tools (~$1120) for the same data, indicating less effective deduplication.

### Root Cause Analysis
1. **Field Mapping Issues**: Original ccost used `uuid + request_id` but real JSONL data has `uuid + sessionId`
2. **Missing Synthetic Filtering**: Competitor tools filter out `<synthetic>` model entries
3. **Complex Fallback Strategy**: Original SHA256-based multi-tier fallbacks were less effective than simple concatenation

### Solution Implementation
1. **Added sessionId field** to UsageData struct to properly read JSONL data
2. **Implemented synthetic model filtering** to exclude `<synthetic>` entries
3. **Simplified hash strategy** to use `message.id:sessionId` concatenation (no fallbacks)
4. **Stricter requirements**: Now requires BOTH message.id AND sessionId for deduplication

### Current Status
- Deduplication rate: ~12% (7349/61515 messages)
- Target: ~18% deduplication rate for competitive parity
- Cost difference: Still showing ~$130 higher than competitor tools

### Next Steps
- Investigate remaining accuracy gap
- Analyze message patterns that competitor tools deduplicate but ccost doesn't
- Consider additional filtering or normalization strategies