import { useDialogMotion } from './useDialogMotion';
import { manageUsage, type UsageReport } from '../ipc';
import { Icon } from './Icon';

export function UsageDialog({
  report,
  close,
  preview = false,
  plan = false,
}: {
  report: UsageReport;
  close: () => void;
  preview?: boolean;
  plan?: boolean;
}) {
  const { modal, dismiss } = useDialogMotion(close);
  const number = new Intl.NumberFormat();
  return (
    <dialog
      ref={modal}
      className="scroll-panel"
      aria-labelledby="usage-title"
      onCancel={(e) => {
        e.preventDefault();
        void dismiss();
      }}
    >
      <div className="dialog-head">
        <h2 id="usage-title">Quill usage</h2>
        <button className="icon-btn" aria-label="Close usage" onClick={dismiss}>
          <Icon name="close" />
        </button>
      </div>
      <div className="panel-scroll usage-body">
        <table>
          <thead>
            <tr>
              <th scope="col">Usage</th>
              <th scope="col">This chat</th>
              <th scope="col">This device</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <th scope="row">Requests</th>
              <td>{number.format(report.chat.requests)}</td>
              <td>{number.format(report.device.requests)}</td>
            </tr>
            {(['inputTokens', 'outputTokens'] as const).map((key) => (
              <tr key={key}>
                <th scope="row">
                  {key === 'inputTokens' ? 'Input tokens' : 'Output tokens'}
                </th>
                {[report.chat, report.device].map((scope, i) => (
                  <td key={i}>
                    {scope.requests && !scope.reported
                      ? '—'
                      : number.format(scope[key])}
                  </td>
                ))}
              </tr>
            ))}
            <tr>
              <th scope="row">Requests with token counts</th>
              <td>{number.format(report.chat.reported)}</td>
              <td>{number.format(report.device.reported)}</td>
            </tr>
          </tbody>
        </table>
        {plan && (
          <a
            href="https://chatgpt.com/settings/usage"
            onClick={(e) => {
              e.preventDefault();
              void manageUsage().catch(() => {});
            }}
          >
            Manage ChatGPT usage
          </a>
        )}
        <p>
          {preview
            ? 'This preview uses sample messages. Token counts are available in the desktop app.'
            : 'Token totals include only counts returned by the API. Older or interrupted requests may have no counts.'}
        </p>
      </div>
    </dialog>
  );
}
