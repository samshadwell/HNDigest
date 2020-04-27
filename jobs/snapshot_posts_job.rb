# frozen_string_literal: true

require_relative '../lib/post_fetcher'
require_relative '../lib/storage_adapter'
require_relative '../configuration'

class SnapshotPostsJob
  LOOKBACK = 2 * 24 * 60 * 60 # 2 days in seconds.
  private_constant :LOOKBACK

  def run(time:)
    # 2x top K in case all the top k were sent yesterday.
    posts = PostFetcher.fetch(top_k: 2 * Configuration::TOP_K_VALUES.max,
                              points: Configuration::POINT_THRESHOLD_VALUES.min,
                              since: time - LOOKBACK)

    # storage = StorageAdapter.new
    # storage.snapshot_posts(posts: posts, time: time)
  end
end
