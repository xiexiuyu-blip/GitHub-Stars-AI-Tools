import type { SubTab } from '@/lib/nav';
import type { AppPage } from '@/types';

type SubTabsProps = {
  tabs: readonly SubTab[];
  active: AppPage;
  onSelect: (page: AppPage) => void;
};

export function SubTabs(props: SubTabsProps) {
  return (
    <div className="sub-tabs" role="tablist">
      {props.tabs.map((tab) => {
        const selected = props.active === tab.key;
        return (
          <button
            key={tab.key}
            type="button"
            role="tab"
            aria-selected={selected}
            onClick={() => props.onSelect(tab.key)}
          >
            {tab.label}
          </button>
        );
      })}
    </div>
  );
}
