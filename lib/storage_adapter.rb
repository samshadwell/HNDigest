# frozen_string_literal: true

require 'aws-sdk'

class StorageAdapter
  TABLE = 'HNDigest'
  private_constant :TABLE

  SNAPSHOT_PARTITION_KEY = 'POSTS_SNAPSHOT'
  private_constant :SNAPSHOT_PARTITION_KEY

  SNAPSHOT_TTL = 30 * 24 * 60 * 60 # Seconds in 30 days.

  def initialize
    @dynamodb = Aws::DynamoDB::Client.new
  end

  def snapshot_posts(posts:, time:)
    datestamp = time.getutc.strftime('%F')
    item = {
      PK: SNAPSHOT_PARTITION_KEY,
      SK: datestamp,
      posts: posts,
      expires_at: time.to_i + SNAPSHOT_TTL
    }

    @dynamodb.put_item(table_name: TABLE, item: item)
  end
end
