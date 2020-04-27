# frozen_string_literal: true

require 'aws-sdk-dynamodb'

class StorageAdapter
  TABLE = 'HNDigest'
  private_constant :TABLE

  SNAPSHOT_PARTITION_KEY = 'POSTS_SNAPSHOT'
  private_constant :SNAPSHOT_PARTITION_KEY

  SNAPSHOT_TTL = 30 * 24 * 60 * 60 # Seconds in 30 days.
  private_constant :SNAPSHOT_TTL

  DIGEST_PARTITION_KEY_PREFIX = 'DIGEST'
  private_constant :DIGEST_PARTITION_KEY_PREFIX

  def initialize
    @dynamodb = Aws::DynamoDB::Client.new
  end

  def snapshot_posts(posts:, date:)
    datestamp = datestamp(date)
    item = {
      PK: SNAPSHOT_PARTITION_KEY,
      SK: datestamp,
      posts: posts,
      expires_at: date.to_i + SNAPSHOT_TTL
    }

    @dynamodb.put_item(table_name: TABLE, item: item)
  end

  def fetch_post_snapshot(date:)
    datestamp = datestamp(date)
    item = fetch_item(
      partition_key: SNAPSHOT_PARTITION_KEY,
      sort_key: datestamp,
    )

    item && item['posts']
  end

  def save_digest(type:, date:, posts:)
    datestamp = datestamp(date)
    item = {
      PK: digest_partition_key(type),
      SK: datestamp,
      posts: posts,
    }

    @dynamodb.put_item(table_name: TABLE, item: item)
  end

  def fetch_digest(type:, date:)
    datestamp = datestamp(date)
    fetch_item(
      partition_key: digest_partition_key(type),
      sort_key: datestamp,
    )
  end

  private

  def datestamp(date)
    date.getutc.strftime('%F')
  end

  def fetch_item(partition_key:, sort_key:)
    @dynamodb.get_item({
      key: {
        PK: partition_key,
        SK: sort_key,
      },
      table_name: TABLE,
    })&.item
  end

  def digest_partition_key(type)
    "#{DIGEST_PARTITION_KEY_PREFIX}##{type}"
  end
end
