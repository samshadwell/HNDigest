# frozen_string_literal: true

require_relative 'post_fetcher'
require_relative '../configuration'

class PostSnapshotter
  LOOKBACK = 2 * 24 * 60 * 60 # 2 days in seconds.
  private_constant :LOOKBACK

  def initialize(storage_adapter:)
    @storage = storage_adapter
  end

  def snapshot(date:)
    # 2x top n in case all the top n were sent yesterday.
    posts = PostFetcher.fetch(top_k: 2 * Configuration::TOP_N_VALUES.max,
                              points: Configuration::POINT_THRESHOLD_VALUES.min,
                              since: date - LOOKBACK)

    @storage.snapshot_posts(posts:, date:)

    posts
  end
end
